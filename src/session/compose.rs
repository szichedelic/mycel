use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::ResolvedBackend;

use super::runtime::{RuntimeKind, RuntimeProvider, RuntimeSession};

/// Runtime provider that runs each session as a Docker Compose project.
///
/// Each session gets its own Compose project directory under
/// `~/.local/share/mycel/compose/<project>-<session>/` with a generated
/// `docker-compose.yml`. The Compose project name is used as the runtime_id
/// for all subsequent lifecycle operations.
pub struct ComposeProvider;

#[allow(dead_code)]
impl ComposeProvider {
    pub fn new() -> Self {
        Self
    }

    fn compose_dir(project_name: &str, session_name: &str) -> Result<PathBuf> {
        let base = dirs::data_dir()
            .context("Could not find data directory")?
            .join("mycel")
            .join("compose")
            .join(format!("{project_name}-{session_name}"));
        Ok(base)
    }

    fn compose_project_name(project_name: &str, session_name: &str) -> String {
        format!("mycel-{project_name}-{session_name}")
    }

    fn generate_compose_file(
        compose_dir: &Path,
        worktree_path: &Path,
        backend: &ResolvedBackend,
    ) -> Result<PathBuf> {
        fs::create_dir_all(compose_dir)
            .context("Failed to create compose project directory")?;

        let file_path = compose_dir.join("docker-compose.yml");

        let mut cmd_parts = vec![backend.command.clone()];
        cmd_parts.extend(backend.args.iter().cloned());
        let command_yaml = serde_json::to_string(&cmd_parts)?;

        let worktree_str = worktree_path.to_string_lossy();

        let yaml = format!(
            r#"services:
  agent:
    image: ubuntu:22.04
    working_dir: /workspace
    stdin_open: true
    tty: true
    volumes:
      - {worktree_str}:/workspace
    command: {command_yaml}
"#
        );

        fs::write(&file_path, yaml)
            .context("Failed to write docker-compose.yml")?;

        Ok(file_path)
    }

    fn run_compose(project_name: &str, compose_dir: &Path, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("docker")
            .arg("compose")
            .arg("-p")
            .arg(project_name)
            .arg("-f")
            .arg(compose_dir.join("docker-compose.yml"))
            .args(args)
            .output()
            .context("Failed to run docker compose")?;
        Ok(output)
    }
}

impl RuntimeProvider for ComposeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Compose
    }

    fn create(
        &self,
        project_name: &str,
        session_name: &str,
        worktree_path: &Path,
        _setup_commands: &[String],
        backend: &ResolvedBackend,
    ) -> Result<RuntimeSession> {
        let compose_project = Self::compose_project_name(project_name, session_name);
        let compose_dir = Self::compose_dir(project_name, session_name)?;

        Self::generate_compose_file(&compose_dir, worktree_path, backend)?;

        let output = Self::run_compose(&compose_project, &compose_dir, &["up", "-d"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("docker compose up failed: {stderr}");
        }

        Ok(RuntimeSession {
            runtime_id: compose_project,
            kind: RuntimeKind::Compose,
        })
    }

    fn is_alive(&self, runtime_id: &str) -> Result<bool> {
        let output = Command::new("docker")
            .args(["compose", "-p", runtime_id, "ps", "--status", "running", "-q"])
            .output()
            .context("Failed to check compose project status")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    fn attach(&self, runtime_id: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(["compose", "-p", runtime_id, "ps", "-q", "agent"])
            .output()
            .context("Failed to find agent container")?;

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if container_id.is_empty() {
            anyhow::bail!("No running agent container found for project {runtime_id}");
        }

        let status = Command::new("docker")
            .args(["attach", &container_id])
            .status()
            .context("Failed to attach to container")?;

        if !status.success() {
            anyhow::bail!("docker attach failed");
        }

        Ok(())
    }

    fn kill(&self, runtime_id: &str) -> Result<()> {
        let status = Command::new("docker")
            .args(["compose", "-p", runtime_id, "down", "--remove-orphans"])
            .status()
            .context("Failed to stop compose project")?;

        if !status.success() {
            anyhow::bail!("docker compose down failed");
        }

        Ok(())
    }

    fn send_keys(&self, runtime_id: &str, text: &str) -> Result<()> {
        let output = Command::new("docker")
            .args(["compose", "-p", runtime_id, "ps", "-q", "agent"])
            .output()
            .context("Failed to find agent container")?;

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if container_id.is_empty() {
            anyhow::bail!("No running agent container found for project {runtime_id}");
        }

        let status = Command::new("docker")
            .args(["exec", "-i", &container_id, "sh", "-c", &format!("echo '{}'", text.replace('\'', "'\\''"))])
            .status()
            .context("Failed to send input to container")?;

        if !status.success() {
            anyhow::bail!("docker exec failed");
        }

        Ok(())
    }

    fn set_label(
        &self,
        _runtime_id: &str,
        _project_name: &str,
        _session_name: &str,
    ) -> Result<()> {
        // Compose containers don't have a tmux-style status bar to label.
        // Labels are tracked via the DB session_runtimes table instead.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_project_name_is_namespaced() {
        let name = ComposeProvider::compose_project_name("myapp", "feat-auth");
        assert_eq!(name, "mycel-myapp-feat-auth");
    }

    #[test]
    fn compose_dir_is_under_data_dir() {
        let dir = ComposeProvider::compose_dir("myapp", "feat-auth").unwrap();
        assert!(dir.ends_with("mycel/compose/myapp-feat-auth"));
    }

    #[test]
    fn generate_compose_file_creates_valid_yaml() {
        let tmp = std::env::temp_dir().join("mycel-test-compose");
        let _ = fs::remove_dir_all(&tmp);

        let backend = ResolvedBackend {
            name: "claude".into(),
            command: "claude".into(),
            args: vec!["--dangerously-skip-permissions".into()],
        };

        let file = ComposeProvider::generate_compose_file(
            &tmp,
            Path::new("/home/user/project"),
            &backend,
        )
        .unwrap();

        assert!(file.exists());
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("services:"));
        assert!(content.contains("agent:"));
        assert!(content.contains("/home/user/project:/workspace"));
        assert!(content.contains("ubuntu:22.04"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
