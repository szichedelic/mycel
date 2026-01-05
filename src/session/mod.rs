use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

const STARTUP_LOGO: &str = "
\x1b[38;2;0;50;80m              ░░░▒▒▒▓▓███▓▓▒▒▒░░░\x1b[0m
\x1b[38;2;0;80;100m           ░▒▓█▀▀             ▀▀█▓▒░\x1b[0m
\x1b[38;2;0;100;110m         ░▓█▀   ·    ·    ·      ▀█▓░\x1b[0m
\x1b[38;2;0;120;120m        ▒█▀  ·    ╲  │  ╱    ·     ▀█▒\x1b[0m
\x1b[38;2;0;140;130m       ▓█   ·   ·──╲─┼─╱──·   ·     █▓\x1b[0m
\x1b[38;2;30;160;135m      ▓█      ╲     ╲│╱     ╱        █▓\x1b[0m
\x1b[38;2;50;180;140m      █▓  · ───●─────●─────●─── ·    ▓█\x1b[0m
\x1b[38;2;30;160;135m      ▓█      ╱     ╱│╲     ╲        █▓\x1b[0m
\x1b[38;2;0;140;130m       ▓█   ·   ·──╱─┼─╲──·   ·     █▓\x1b[0m
\x1b[38;2;0;120;120m        ▒█▀  ·    ╱  │  ╲    ·     ▀█▒\x1b[0m
\x1b[38;2;0;100;110m         ░▓█▄   ·    ·    ·      ▄█▓░\x1b[0m
\x1b[38;2;0;80;100m           ░▒▓█▄▄             ▄▄█▓▒░\x1b[0m
\x1b[38;2;0;50;80m              ░░░▒▒▒▓▓███▓▓▒▒▒░░░\x1b[0m

\x1b[38;2;0;180;180m                  M Y C E L\x1b[0m
\x1b[38;2;100;100;100m          the network beneath your code\x1b[0m
";

pub struct SessionManager;

impl SessionManager {
    pub fn new() -> Self {
        Self
    }

    /// Create a new tmux session with Claude Code running in the given worktree
    pub fn create(
        &self,
        project_name: &str,
        session_name: &str,
        worktree_path: &Path,
        setup_commands: &[String],
    ) -> Result<String> {
        let tmux_session_name = format!("mycel-{project_name}-{session_name}");

        // Create tmux session in detached mode, starting in the worktree directory
        let status = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &tmux_session_name,
                "-c",
                &worktree_path.to_string_lossy(),
            ])
            .status()
            .context("Failed to create tmux session")?;

        if !status.success() {
            anyhow::bail!("tmux new-session failed");
        }

        // Build command: setup commands -> logo -> claude
        let logo_escaped = STARTUP_LOGO.replace("'", "'\\''");
        let setup_str = if setup_commands.is_empty() {
            String::new()
        } else {
            setup_commands.join(" && ") + " && "
        };
        let logo_cmd =
            format!("{setup_str}clear && printf '{logo_escaped}' && sleep 1.5 && clear && claude");
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &tmux_session_name, &logo_cmd, "Enter"])
            .status()
            .context("Failed to start Claude in tmux session")?;

        if !status.success() {
            anyhow::bail!("Failed to start Claude Code in session");
        }

        Ok(tmux_session_name)
    }

    /// Check if a tmux session is still alive
    pub fn is_alive(&self, tmux_session: &str) -> Result<bool> {
        let output = Command::new("tmux")
            .args(["has-session", "-t", tmux_session])
            .output()
            .context("Failed to check tmux session")?;

        Ok(output.status.success())
    }

    /// Attach to an existing tmux session
    pub fn attach(&self, tmux_session: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["attach-session", "-t", tmux_session])
            .status()
            .context("Failed to attach to tmux session")?;

        if !status.success() {
            anyhow::bail!("tmux attach failed");
        }

        Ok(())
    }

    /// Kill a tmux session
    pub fn kill(&self, tmux_session: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["kill-session", "-t", tmux_session])
            .status()
            .context("Failed to kill tmux session")?;

        if !status.success() {
            anyhow::bail!("tmux kill-session failed");
        }

        Ok(())
    }
}
