use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::ResolvedBackend;

use super::runtime::{RuntimeKind, RuntimeProvider, RuntimeSession};

/// Runtime provider that runs Docker Compose sessions on a remote host via SSH.
///
/// All Docker commands are executed with `DOCKER_HOST=ssh://<host>` so the local
/// Docker CLI transparently targets the remote daemon. The compose file is
/// generated locally and copied to the remote host via `scp`.
pub struct RemoteProvider {
    /// SSH-style Docker host, e.g. `ssh://user@host` or `ssh://host`.
    docker_host: String,
}

impl RemoteProvider {
    pub fn new(docker_host: String) -> Self {
        Self { docker_host }
    }

    /// Derive an SSH destination from the docker_host for scp/ssh commands.
    /// Strips the `ssh://` prefix.
    fn ssh_dest(&self) -> &str {
        self.docker_host
            .strip_prefix("ssh://")
            .unwrap_or(&self.docker_host)
    }

    fn compose_project_name(project_name: &str, session_name: &str) -> String {
        format!("mycel-{project_name}-{session_name}")
    }

    fn local_compose_dir(project_name: &str, session_name: &str) -> Result<PathBuf> {
        let base = dirs::data_dir()
            .context("Could not find data directory")?
            .join("mycel")
            .join("remote-compose")
            .join(format!("{project_name}-{session_name}"));
        Ok(base)
    }

    fn remote_compose_dir(project_name: &str, session_name: &str) -> String {
        format!("/tmp/mycel-compose/{project_name}-{session_name}")
    }

    fn generate_compose_file(
        compose_dir: &Path,
        worktree_path: &Path,
        backend: &ResolvedBackend,
    ) -> Result<PathBuf> {
        fs::create_dir_all(compose_dir).context("Failed to create local compose directory")?;

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

        fs::write(&file_path, yaml).context("Failed to write docker-compose.yml")?;

        Ok(file_path)
    }

    /// Copy the compose file to the remote host via scp.
    fn sync_compose_file(&self, local_path: &Path, remote_dir: &str) -> Result<()> {
        let ssh_dest = self.ssh_dest();

        // Create remote directory
        let status = Command::new("ssh")
            .args([ssh_dest, "mkdir", "-p", remote_dir])
            .status()
            .context("Failed to create remote directory via SSH")?;
        if !status.success() {
            anyhow::bail!("ssh mkdir failed on {ssh_dest}");
        }

        // Copy compose file
        let remote_path = format!("{ssh_dest}:{remote_dir}/docker-compose.yml");
        let local_str = local_path.to_string_lossy().to_string();
        let status = Command::new("scp")
            .args([&local_str, &remote_path])
            .status()
            .context("Failed to scp compose file to remote host")?;
        if !status.success() {
            anyhow::bail!("scp failed to {ssh_dest}");
        }

        Ok(())
    }

    /// Run a docker compose command targeting the remote host.
    fn run_remote_compose(
        &self,
        project_name: &str,
        remote_dir: &str,
        args: &[&str],
    ) -> Result<std::process::Output> {
        let remote_file = format!("{remote_dir}/docker-compose.yml");
        let output = Command::new("docker")
            .env("DOCKER_HOST", &self.docker_host)
            .arg("compose")
            .arg("-p")
            .arg(project_name)
            .arg("-f")
            .arg(&remote_file)
            .args(args)
            .output()
            .context("Failed to run docker compose on remote host")?;
        Ok(output)
    }

    /// Run a docker command targeting the remote host (non-compose).
    fn run_remote_docker(&self, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("docker")
            .env("DOCKER_HOST", &self.docker_host)
            .args(args)
            .output()
            .context("Failed to run docker on remote host")?;
        Ok(output)
    }
}

impl RuntimeProvider for RemoteProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Remote
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
        let local_dir = Self::local_compose_dir(project_name, session_name)?;
        let remote_dir = Self::remote_compose_dir(project_name, session_name);

        let local_file = Self::generate_compose_file(&local_dir, worktree_path, backend)?;
        self.sync_compose_file(&local_file, &remote_dir)?;

        let output = self.run_remote_compose(&compose_project, &remote_dir, &["up", "-d"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("docker compose up failed on remote: {stderr}");
        }

        Ok(RuntimeSession {
            runtime_id: compose_project,
            kind: RuntimeKind::Remote,
        })
    }

    fn is_alive(&self, runtime_id: &str) -> Result<bool> {
        let output = Command::new("docker")
            .env("DOCKER_HOST", &self.docker_host)
            .args([
                "compose", "-p", runtime_id, "ps", "--status", "running", "-q",
            ])
            .output()
            .context("Failed to check remote compose project status")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    fn attach(&self, runtime_id: &str) -> Result<()> {
        let output = Command::new("docker")
            .env("DOCKER_HOST", &self.docker_host)
            .args(["compose", "-p", runtime_id, "ps", "-q", "agent"])
            .output()
            .context("Failed to find remote agent container")?;

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if container_id.is_empty() {
            anyhow::bail!("No running agent container found on remote for project {runtime_id}");
        }

        let status = Command::new("docker")
            .env("DOCKER_HOST", &self.docker_host)
            .args(["attach", &container_id])
            .status()
            .context("Failed to attach to remote container")?;

        if !status.success() {
            anyhow::bail!("docker attach to remote failed");
        }

        Ok(())
    }

    fn kill(&self, runtime_id: &str) -> Result<()> {
        let status = Command::new("docker")
            .env("DOCKER_HOST", &self.docker_host)
            .args(["compose", "-p", runtime_id, "down", "--remove-orphans"])
            .status()
            .context("Failed to stop remote compose project")?;

        if !status.success() {
            anyhow::bail!("docker compose down on remote failed");
        }

        Ok(())
    }

    fn send_keys(&self, runtime_id: &str, text: &str) -> Result<()> {
        let output = self.run_remote_docker(&["compose", "-p", runtime_id, "ps", "-q", "agent"])?;

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if container_id.is_empty() {
            anyhow::bail!("No running agent container found on remote for project {runtime_id}");
        }

        let escaped = text.replace('\'', "'\\''");
        let output = self.run_remote_docker(&[
            "exec",
            "-i",
            &container_id,
            "sh",
            "-c",
            &format!("echo '{escaped}'"),
        ])?;

        if !output.status.success() {
            anyhow::bail!("docker exec on remote failed");
        }

        Ok(())
    }

    fn set_label(&self, _runtime_id: &str, _project_name: &str, _session_name: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_compose_project_name_is_namespaced() {
        let name = RemoteProvider::compose_project_name("myapp", "feat-auth");
        assert_eq!(name, "mycel-myapp-feat-auth");
    }

    #[test]
    fn ssh_dest_strips_prefix() {
        let provider = RemoteProvider::new("ssh://user@example.com".into());
        assert_eq!(provider.ssh_dest(), "user@example.com");
    }

    #[test]
    fn ssh_dest_handles_no_prefix() {
        let provider = RemoteProvider::new("user@example.com".into());
        assert_eq!(provider.ssh_dest(), "user@example.com");
    }

    #[test]
    fn remote_compose_dir_is_under_tmp() {
        let dir = RemoteProvider::remote_compose_dir("myapp", "feat-auth");
        assert_eq!(dir, "/tmp/mycel-compose/myapp-feat-auth");
    }

    #[test]
    fn local_compose_dir_is_under_data_dir() {
        let dir = RemoteProvider::local_compose_dir("myapp", "feat-auth").unwrap();
        assert!(dir.ends_with("mycel/remote-compose/myapp-feat-auth"));
    }

    #[test]
    fn generate_compose_file_creates_yaml() {
        let tmp = std::env::temp_dir().join("mycel-test-remote-compose");
        let _ = fs::remove_dir_all(&tmp);

        let backend = ResolvedBackend {
            name: "claude".into(),
            command: "claude".into(),
            args: vec!["--dangerously-skip-permissions".into()],
        };

        let file =
            RemoteProvider::generate_compose_file(&tmp, Path::new("/home/user/project"), &backend)
                .unwrap();

        assert!(file.exists());
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("services:"));
        assert!(content.contains("agent:"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn provider_reports_remote_kind() {
        let provider = RemoteProvider::new("ssh://user@host".into());
        assert_eq!(provider.kind(), RuntimeKind::Remote);
    }
}
