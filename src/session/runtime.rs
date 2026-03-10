use anyhow::Result;
use std::fmt;
use std::path::Path;

use crate::config::ResolvedBackend;

/// Identifies the type of runtime backing a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Tmux,
    Compose,
    Remote,
}

impl RuntimeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeKind::Tmux => "tmux",
            RuntimeKind::Compose => "compose",
            RuntimeKind::Remote => "remote",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "tmux" => Some(RuntimeKind::Tmux),
            "compose" => Some(RuntimeKind::Compose),
            "remote" => Some(RuntimeKind::Remote),
            _ => None,
        }
    }
}

impl fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Handle returned by a runtime after creating a session.
#[allow(dead_code)]
pub struct RuntimeSession {
    /// Provider-specific session identifier (e.g. tmux session name).
    pub runtime_id: String,
    /// Which runtime kind created this session.
    pub kind: RuntimeKind,
}

/// Common interface for session runtime providers.
///
/// Each provider (tmux, future Docker, etc.) implements this trait so the rest
/// of Mycel can manage sessions without knowing the underlying runtime.
pub trait RuntimeProvider: Send + Sync {
    /// Which kind of runtime this provider represents.
    fn kind(&self) -> RuntimeKind;

    /// Create a new runtime session.
    ///
    /// Returns a `RuntimeSession` containing the provider-specific identifier
    /// that callers must persist for later operations.
    fn create(
        &self,
        project_name: &str,
        session_name: &str,
        worktree_path: &Path,
        setup_commands: &[String],
        backend: &ResolvedBackend,
    ) -> Result<RuntimeSession>;

    /// Check whether a previously created session is still running.
    fn is_alive(&self, runtime_id: &str) -> Result<bool>;

    /// Attach the current terminal to a running session.
    fn attach(&self, runtime_id: &str) -> Result<()>;

    /// Terminate a running session.
    fn kill(&self, runtime_id: &str) -> Result<()>;

    /// Send text input to a running session.
    fn send_keys(&self, runtime_id: &str, text: &str) -> Result<()>;

    /// Set a human-readable label on the session (best-effort).
    fn set_label(&self, runtime_id: &str, project_name: &str, session_name: &str) -> Result<()>;
}
