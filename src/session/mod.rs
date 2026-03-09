use anyhow::Result;
use std::path::Path;

use crate::config::ResolvedBackend;

pub mod compose;
pub mod runtime;
pub mod tmux;

#[allow(unused_imports)]
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
