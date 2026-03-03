use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use tracing::warn;

use crate::config::ResolvedBackend;

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

    /// Create a new tmux session with the selected backend running in the given worktree
    pub fn create(
        &self,
        project_name: &str,
        session_name: &str,
        worktree_path: &Path,
        setup_commands: &[String],
        backend: &ResolvedBackend,
    ) -> Result<String> {
        let tmux_session_name = format!("mycel-{project_name}-{session_name}");

        // Kill any stale session with this name (e.g. after force shutdown)
        if self.is_alive(&tmux_session_name).unwrap_or(false) {
            let _ = self.kill(&tmux_session_name);
        }

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

        if let Err(err) =
            self.set_session_label(&tmux_session_name, project_name, session_name)
        {
            warn!("Failed to set tmux session label: {err}");
        }

        // Build command: setup commands -> logo -> backend
        let logo_escaped = STARTUP_LOGO.replace("'", "'\\''");
        let setup_str = if setup_commands.is_empty() {
            String::new()
        } else {
            setup_commands.join(" && ") + " && "
        };
        let backend_cmd = build_backend_command(backend);
        let logo_cmd = format!(
            "{setup_str}clear && printf '{logo_escaped}' && sleep 1.5 && clear && {backend_cmd}"
        );
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &tmux_session_name, &logo_cmd, "Enter"])
            .status()
            .context("Failed to start backend in tmux session")?;

        if !status.success() {
            anyhow::bail!("Failed to start backend session");
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

    /// Send an initial prompt to the running session
    pub fn send_prompt(&self, tmux_session: &str, prompt: &str) -> Result<()> {
        for line in prompt.lines() {
            let line = line.trim_end_matches('\r');
            let mut args = vec!["send-keys", "-t", tmux_session];
            if !line.is_empty() {
                args.push(line);
            }
            args.push("Enter");

            let status = Command::new("tmux")
                .args(&args)
                .status()
                .context("Failed to send prompt to tmux session")?;

            if !status.success() {
                anyhow::bail!("tmux send-keys failed");
            }
        }

        Ok(())
    }

    pub fn set_session_label(
        &self,
        tmux_session: &str,
        project_name: &str,
        session_name: &str,
    ) -> Result<()> {
        let label = format!(
            "mycel: {}/{}",
            tmux_escape_format(project_name),
            tmux_escape_format(session_name)
        );
        let base_status_left = tmux_get_global_option("status-left")?;
        let status_left = if base_status_left.is_empty() {
            format!(" {label} ")
        } else {
            format!(" {label} | {base_status_left}")
        };

        let base_left_len = tmux_get_global_option("status-left-length")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let desired_len = label.chars().count() + 2;
        let status_left_len = base_left_len.max(desired_len).to_string();

        tmux_set_session_option(tmux_session, "status-left", &status_left)?;
        tmux_set_session_option(tmux_session, "status-left-length", &status_left_len)?;
        tmux_set_session_option(tmux_session, "set-titles", "on")?;
        tmux_set_session_option(tmux_session, "set-titles-string", &label)?;

        Ok(())
    }
}

fn build_backend_command(backend: &ResolvedBackend) -> String {
    let mut parts = Vec::with_capacity(backend.args.len() + 1);
    parts.push(shell_escape(&backend.command));
    for arg in &backend.args {
        parts.push(shell_escape(arg));
    }
    parts.join(" ")
}

fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn tmux_get_global_option(option: &str) -> Result<String> {
    let output = Command::new("tmux")
        .args(["show-option", "-gqv", option])
        .output()
        .context("Failed to read tmux option")?;
    if !output.status.success() {
        return Ok(String::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn tmux_set_session_option(tmux_session: &str, option: &str, value: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["set-option", "-t", tmux_session, option, value])
        .status()
        .context("Failed to set tmux option")?;
    if !status.success() {
        anyhow::bail!("tmux set-option failed");
    }
    Ok(())
}

fn tmux_escape_format(value: &str) -> String {
    value.replace('#', "##")
}
