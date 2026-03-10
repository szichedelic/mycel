use anyhow::{bail, Result};

use crate::db::{Database, Host};

/// Pick the best available host for a new remote session.
///
/// Strategy: least-loaded among enabled hosts that haven't hit max_sessions.
#[allow(dead_code)]
pub fn pick_host(db: &Database) -> Result<Host> {
    let hosts = db.list_hosts()?;
    let enabled: Vec<_> = hosts.into_iter().filter(|h| h.enabled).collect();

    if enabled.is_empty() {
        bail!("No remote hosts registered. Add one with: mycel host add <name> <docker_host>");
    }

    let mut best: Option<(Host, i64)> = None;

    for host in enabled {
        let count = db.count_sessions_on_host(&host.docker_host)?;
        if count >= host.max_sessions {
            continue;
        }
        match &best {
            None => best = Some((host, count)),
            Some((_, best_count)) if count < *best_count => {
                best = Some((host, count));
            }
            _ => {}
        }
    }

    match best {
        Some((host, _)) => Ok(host),
        None => bail!("All remote hosts are at capacity"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn pick_host_errors_with_no_hosts() {
        let db = in_memory_db();
        assert!(pick_host(&db).is_err());
    }

    #[test]
    fn pick_host_returns_only_enabled_host() {
        let db = in_memory_db();
        db.add_host("devbox", "ssh://user@devbox", 4).unwrap();
        let host = pick_host(&db).unwrap();
        assert_eq!(host.name, "devbox");
    }

    #[test]
    fn pick_host_skips_disabled_hosts() {
        let db = in_memory_db();
        db.add_host("devbox", "ssh://user@devbox", 4).unwrap();
        db.set_host_enabled("devbox", false).unwrap();
        assert!(pick_host(&db).is_err());
    }

    #[test]
    fn pick_host_selects_least_loaded() {
        let db = in_memory_db();
        db.add_host("busy", "ssh://busy", 4).unwrap();
        db.add_host("idle", "ssh://idle", 4).unwrap();

        // Simulate load on "busy" by adding a runtime pointing to its docker_host
        let pid = db
            .add_project("test", std::path::Path::new("/tmp/test"))
            .unwrap();
        let sid = db
            .add_session(&crate::db::NewSession {
                project_id: pid,
                name: "s1",
                branch_name: "s1",
                worktree_path: std::path::Path::new("/tmp/wt"),
                tmux_session: "mycel-test-s1",
                runtime_kind: "remote",
                backend: "claude",
                note: None,
            })
            .unwrap();
        db.replace_session_runtime(&crate::db::NewSessionRuntime {
            session_id: sid,
            provider: "remote",
            host: "ssh://busy",
            runtime_ref: "mycel-test-s1",
            compose_project: None,
            state: "running",
        })
        .unwrap();

        let host = pick_host(&db).unwrap();
        assert_eq!(host.name, "idle");
    }
}
