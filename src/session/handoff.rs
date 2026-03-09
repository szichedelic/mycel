use anyhow::{Context, Result};

use crate::config::ResolvedBackend;
use crate::db::{Database, NewSessionRuntime, Session};

use super::runtime::RuntimeKind;
use super::SessionManager;

/// Describes the destination for a session handoff.
pub struct HandoffTarget {
    pub kind: RuntimeKind,
    /// For remote targets, the SSH docker host (e.g. "ssh://user@host").
    /// For local targets, "local".
    pub host: String,
}

/// Result of a completed handoff.
pub struct HandoffResult {
    pub new_runtime_id: String,
    pub new_kind: RuntimeKind,
    #[allow(dead_code)]
    pub new_host: String,
}

/// Execute a session handoff: stop the source runtime, create on the destination,
/// and update all database records to reflect the new runtime.
///
/// The session's identity (name, branch, worktree, note, backend) is preserved.
/// Only the runtime backing changes.
pub fn handoff_session(
    db: &Database,
    session: &Session,
    project_name: &str,
    target: &HandoffTarget,
    backend: &ResolvedBackend,
    setup_commands: &[String],
) -> Result<HandoffResult> {
    let source_kind = RuntimeKind::from_str(&session.runtime_kind).unwrap_or(RuntimeKind::Tmux);

    // 1. Stop the source runtime (best-effort — it may already be stopped)
    let source_sm = SessionManager::for_kind(source_kind);
    if source_sm.is_alive(&session.tmux_session).unwrap_or(false) {
        source_sm
            .kill(&session.tmux_session)
            .context("Failed to stop source runtime during handoff")?;
    }

    // 2. Create the new runtime on the destination
    let dest_sm = SessionManager::for_kind_with_host(target.kind, &target.host);
    let new_runtime_id = dest_sm
        .create(
            project_name,
            &session.name,
            &session.worktree_path,
            setup_commands,
            backend,
        )
        .context("Failed to create destination runtime during handoff")?;

    // 3. Update the sessions table
    db.update_session_runtime_kind(session.id, target.kind.as_str(), &new_runtime_id)?;

    // 4. Replace session_runtimes row
    db.replace_session_runtime(&NewSessionRuntime {
        session_id: session.id,
        provider: target.kind.as_str(),
        host: &target.host,
        runtime_ref: &new_runtime_id,
        compose_project: None,
        state: "running",
    })?;

    Ok(HandoffResult {
        new_runtime_id,
        new_kind: target.kind,
        new_host: target.host.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_target_local_compose() {
        let target = HandoffTarget {
            kind: RuntimeKind::Compose,
            host: "local".to_string(),
        };
        assert_eq!(target.kind.as_str(), "compose");
        assert_eq!(target.host, "local");
    }

    #[test]
    fn handoff_target_remote() {
        let target = HandoffTarget {
            kind: RuntimeKind::Remote,
            host: "ssh://user@devbox".to_string(),
        };
        assert_eq!(target.kind.as_str(), "remote");
        assert_eq!(target.host, "ssh://user@devbox");
    }
}
