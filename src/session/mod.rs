use anyhow::Result;
use std::path::Path;

use crate::config::ResolvedBackend;

pub mod compose;
pub mod runtime;
pub mod tmux;

pub use compose::ComposeProvider;
pub use runtime::{RuntimeKind, RuntimeProvider};
pub use tmux::TmuxProvider;

pub struct SessionManager {
    provider: Box<dyn RuntimeProvider>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            provider: Box::new(TmuxProvider::new()),
        }
    }

    /// Create a SessionManager with the correct provider for a stored runtime kind.
    pub fn for_kind(kind: RuntimeKind) -> Self {
        let provider: Box<dyn RuntimeProvider> = match kind {
            RuntimeKind::Tmux => Box::new(TmuxProvider::new()),
            RuntimeKind::Compose => Box::new(ComposeProvider::new()),
        };
        Self { provider }
    }

    /// Create a SessionManager from a runtime_kind string stored in the database.
    /// Falls back to tmux if the string is unrecognised.
    pub fn for_kind_str(kind_str: &str) -> Self {
        let kind = RuntimeKind::from_str(kind_str).unwrap_or(RuntimeKind::Tmux);
        Self::for_kind(kind)
    }

    #[allow(dead_code)]
    pub fn with_provider(provider: Box<dyn RuntimeProvider>) -> Self {
        Self { provider }
    }

    pub fn kind(&self) -> RuntimeKind {
        self.provider.kind()
    }

    pub fn create(
        &self,
        project_name: &str,
        session_name: &str,
        worktree_path: &Path,
        setup_commands: &[String],
        backend: &ResolvedBackend,
    ) -> Result<String> {
        let session = self.provider.create(
            project_name,
            session_name,
            worktree_path,
            setup_commands,
            backend,
        )?;
        Ok(session.runtime_id)
    }

    pub fn is_alive(&self, runtime_id: &str) -> Result<bool> {
        self.provider.is_alive(runtime_id)
    }

    pub fn attach(&self, runtime_id: &str) -> Result<()> {
        self.provider.attach(runtime_id)
    }

    pub fn kill(&self, runtime_id: &str) -> Result<()> {
        self.provider.kill(runtime_id)
    }

    pub fn send_prompt(&self, runtime_id: &str, prompt: &str) -> Result<()> {
        self.provider.send_keys(runtime_id, prompt)
    }

    pub fn set_session_label(
        &self,
        runtime_id: &str,
        project_name: &str,
        session_name: &str,
    ) -> Result<()> {
        self.provider.set_label(runtime_id, project_name, session_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_kind_returns_correct_provider() {
        let sm = SessionManager::for_kind(RuntimeKind::Tmux);
        assert_eq!(sm.kind(), RuntimeKind::Tmux);

        let sm = SessionManager::for_kind(RuntimeKind::Compose);
        assert_eq!(sm.kind(), RuntimeKind::Compose);
    }

    #[test]
    fn for_kind_str_parses_known_kinds() {
        assert_eq!(SessionManager::for_kind_str("tmux").kind(), RuntimeKind::Tmux);
        assert_eq!(SessionManager::for_kind_str("compose").kind(), RuntimeKind::Compose);
    }

    #[test]
    fn for_kind_str_falls_back_to_tmux() {
        assert_eq!(SessionManager::for_kind_str("unknown").kind(), RuntimeKind::Tmux);
        assert_eq!(SessionManager::for_kind_str("").kind(), RuntimeKind::Tmux);
    }
}
