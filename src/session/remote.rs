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

    fn remote_workspace_dir(project_name: &str, session_name: &str) -> String {
        format!("mycel-workspaces/{project_name}/{session_name}")
    }

    /// Rsync the local worktree to the remote host.
    /// Get the remote user's home directory.
    fn remote_home(&self) -> Result<String> {
        let ssh_dest = self.ssh_dest();
        let output = Command::new("ssh")
            .args([ssh_dest, "echo", "$HOME"])
            .output()
            .context("Failed to get remote home directory")?;
        if !output.status.success() {
            anyhow::bail!("Failed to get remote home directory on {ssh_dest}");
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn rsync_worktree(
        &self,
        local_path: &Path,
        project_name: &str,
        session_name: &str,
    ) -> Result<String> {
        let relative_dir = Self::remote_workspace_dir(project_name, session_name);
        let ssh_dest = self.ssh_dest();
        let home = self.remote_home()?;
        let absolute_dir = format!("{home}/{relative_dir}");

        let status = Command::new("ssh")
            .args([ssh_dest, "mkdir", "-p", &absolute_dir])
            .status()
            .context("Failed to create remote workspace directory via SSH")?;
        if !status.success() {
            anyhow::bail!("ssh mkdir for workspace failed on {ssh_dest}");
        }

        // rsync the worktree (trailing slash on source means "contents of")
        let local_str = format!("{}/", local_path.to_string_lossy());
        let remote_str = format!("{ssh_dest}:{absolute_dir}/");
        let status = Command::new("rsync")
            .args(["-az", "--delete", &local_str, &remote_str])
            .status()
            .context("Failed to rsync worktree to remote host")?;
        if !status.success() {
            anyhow::bail!("rsync to {ssh_dest}:{absolute_dir} failed");
        }

        Ok(absolute_dir)
    }

    fn generate_compose_file(
        compose_dir: &Path,
        remote_workspace: &str,
        setup_commands: &[String],
        backend: &ResolvedBackend,
    ) -> Result<PathBuf> {
        fs::create_dir_all(compose_dir).context("Failed to create local compose directory")?;

        let file_path = compose_dir.join("docker-compose.yml");

        let mut cmd_parts = vec![backend.command.clone()];
        cmd_parts.extend(backend.args.iter().cloned());
        let backend_cmd = shell_escape_args(&cmd_parts);

        let entrypoint = build_remote_entrypoint(setup_commands, &backend_cmd);

        let yaml = format!(
            r#"services:
  agent:
    image: mycel-agent:latest
    working_dir: /workspace
    stdin_open: true
    tty: true
    volumes:
      - {remote_workspace}:/workspace
    entrypoint: ["/bin/bash", "-c"]
    command:
      - |
        {entrypoint}
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

    /// Run a docker compose command on the remote host via SSH.
    fn run_remote_compose(
        &self,
        project_name: &str,
        remote_dir: &str,
        args: &[&str],
    ) -> Result<std::process::Output> {
        let ssh_dest = self.ssh_dest();
        let remote_file = format!("{remote_dir}/docker-compose.yml");
        let docker_args = format!(
            "docker compose -p {} -f {} {}",
            project_name,
            remote_file,
            args.join(" ")
        );
        let output = Command::new("ssh")
            .args([
                "-o",
                "ConnectTimeout=5",
                "-o",
                "ServerAliveInterval=5",
                "-o",
                "ServerAliveCountMax=1",
                ssh_dest,
                &docker_args,
            ])
            .output()
            .context("Failed to run docker compose on remote host via SSH")?;
        Ok(output)
    }

    /// Run a docker command on the remote host via SSH.
    fn run_remote_docker(&self, args: &[&str]) -> Result<std::process::Output> {
        let ssh_dest = self.ssh_dest();
        let docker_args = format!("docker {}", args.join(" "));
        let output = Command::new("ssh")
            .args([
                "-o",
                "ConnectTimeout=5",
                "-o",
                "ServerAliveInterval=5",
                "-o",
                "ServerAliveCountMax=1",
                ssh_dest,
                &docker_args,
            ])
            .output()
            .context("Failed to run docker on remote host via SSH")?;
        Ok(output)
    }

    /// Stop all mycel containers and remove workspace data on the remote host.
    pub fn cleanup_host(&self) -> Result<()> {
        let ssh_dest = self.ssh_dest();

        // Stop all mycel-* compose projects
        let output = self.run_remote_docker(&[
            "ps",
            "-a",
            "--filter",
            "label=com.docker.compose.project",
            "--format",
            "{{.Labels}}",
        ])?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut projects_seen = std::collections::HashSet::new();
        for line in stdout.lines() {
            for label in line.split(',') {
                if let Some(project) = label.strip_prefix("com.docker.compose.project=") {
                    if project.starts_with("mycel-") {
                        projects_seen.insert(project.to_string());
                    }
                }
            }
        }

        for project in &projects_seen {
            let _ = Command::new("ssh")
                .args([
                    ssh_dest,
                    "docker",
                    "compose",
                    "-p",
                    project,
                    "down",
                    "--remove-orphans",
                ])
                .status();
        }

        let _ = Command::new("ssh")
            .args([
                ssh_dest,
                "rm",
                "-rf",
                "~/mycel-workspaces/",
                "/tmp/mycel-compose/",
            ])
            .status();

        Ok(())
    }
}

/// Build an entrypoint script that installs deps, runs setup commands, then launches the backend.
/// Used by the local compose provider with bare ubuntu:22.04.
pub fn build_entrypoint(setup_commands: &[String], backend_cmd: &str) -> String {
    let mut lines = Vec::new();

    lines
        .push("apt-get update -qq && apt-get install -y -qq curl git > /dev/null 2>&1".to_string());

    for cmd in setup_commands {
        lines.push(cmd.clone());
    }

    lines.push(format!("exec {backend_cmd}"));

    lines.join(" && \\\n        ")
}

/// Build an entrypoint for the mycel-agent image (deps pre-installed).
/// Runs setup commands, then `claude login && exec backend`.
pub fn build_remote_entrypoint(setup_commands: &[String], backend_cmd: &str) -> String {
    let mut lines = Vec::new();

    for cmd in setup_commands {
        lines.push(cmd.clone());
    }

    let login_cmd = if backend_cmd.starts_with("claude") {
        "claude login"
    } else if backend_cmd.starts_with("codex") {
        "codex login"
    } else {
        ""
    };

    if !login_cmd.is_empty() {
        lines.push(login_cmd.to_string());
    }

    lines.push(format!("exec {backend_cmd}"));

    lines.join(" && \\\n        ")
}

/// Shell-escape a list of args into a single command string.
pub fn shell_escape_args(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.contains(' ') || a.contains('\'') || a.contains('"') || a.contains('$') {
                format!("'{}'", a.replace('\'', "'\\''"))
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
        setup_commands: &[String],
        backend: &ResolvedBackend,
    ) -> Result<RuntimeSession> {
        let compose_project = Self::compose_project_name(project_name, session_name);
        let local_dir = Self::local_compose_dir(project_name, session_name)?;
        let remote_dir = Self::remote_compose_dir(project_name, session_name);

        // Rsync worktree to remote host
        let remote_workspace = self.rsync_worktree(worktree_path, project_name, session_name)?;

        let local_file =
            Self::generate_compose_file(&local_dir, &remote_workspace, setup_commands, backend)?;
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
        let output = self.run_remote_docker(&[
            "compose", "-p", runtime_id, "ps", "--status", "running", "-q",
        ])?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    fn attach(&self, runtime_id: &str) -> Result<()> {
        let output = self.run_remote_docker(&["compose", "-p", runtime_id, "ps", "-q", "agent"])?;

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if container_id.is_empty() {
            anyhow::bail!("No running agent container found on remote for project {runtime_id}");
        }

        // Attach interactively via SSH
        let ssh_dest = self.ssh_dest();
        let status = Command::new("ssh")
            .args(["-t", ssh_dest, "docker", "attach", &container_id])
            .status()
            .context("Failed to attach to remote container")?;

        if !status.success() {
            anyhow::bail!("docker attach to remote failed");
        }

        Ok(())
    }

    fn kill(&self, runtime_id: &str) -> Result<()> {
        let ssh_dest = self.ssh_dest();
        let status = Command::new("ssh")
            .args([
                ssh_dest,
                "docker",
                "compose",
                "-p",
                runtime_id,
                "down",
                "--remove-orphans",
            ])
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
    fn remote_workspace_dir_is_under_opt() {
        let dir = RemoteProvider::remote_workspace_dir("myapp", "feat-auth");
        assert_eq!(dir, "mycel-workspaces/myapp/feat-auth");
    }

    #[test]
    fn local_compose_dir_is_under_data_dir() {
        let dir = RemoteProvider::local_compose_dir("myapp", "feat-auth").unwrap();
        assert!(dir.ends_with("mycel/remote-compose/myapp-feat-auth"));
    }

    #[test]
    fn generate_compose_file_creates_yaml_with_remote_workspace() {
        let tmp = std::env::temp_dir().join("mycel-test-remote-compose");
        let _ = fs::remove_dir_all(&tmp);

        let backend = ResolvedBackend {
            name: "claude".into(),
            command: "claude".into(),
            args: vec!["--dangerously-skip-permissions".into()],
        };

        let file = RemoteProvider::generate_compose_file(
            &tmp,
            "mycel-workspaces/myapp/feat-auth",
            &["curl -fsSL https://example.com/install.sh | sh".to_string()],
            &backend,
        )
        .unwrap();

        assert!(file.exists());
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("services:"));
        assert!(content.contains("agent:"));
        assert!(content.contains("mycel-workspaces/myapp/feat-auth:/workspace"));
        assert!(content.contains("mycel-agent:latest"));
        assert!(!content.contains("/home/user/project"));
        assert!(!content.contains("apt-get update"));
        assert!(content.contains("curl -fsSL"));
        assert!(content.contains("claude login"));
        assert!(content.contains("claude"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn generate_compose_file_no_setup_commands() {
        let tmp = std::env::temp_dir().join("mycel-test-remote-compose-no-setup");
        let _ = fs::remove_dir_all(&tmp);

        let backend = ResolvedBackend {
            name: "codex".into(),
            command: "codex".into(),
            args: vec![],
        };

        let file =
            RemoteProvider::generate_compose_file(&tmp, "mycel-workspaces/p/s", &[], &backend)
                .unwrap();

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("codex login"));
        assert!(content.contains("exec codex"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn provider_reports_remote_kind() {
        let provider = RemoteProvider::new("ssh://user@host".into());
        assert_eq!(provider.kind(), RuntimeKind::Remote);
    }

    #[test]
    fn build_entrypoint_includes_deps_and_setup() {
        let result = build_entrypoint(
            &["pip install foo".to_string(), "npm install".to_string()],
            "claude --dangerously-skip-permissions",
        );
        assert!(result.contains("apt-get update"));
        assert!(result.contains("pip install foo"));
        assert!(result.contains("npm install"));
        assert!(result.contains("exec claude --dangerously-skip-permissions"));
    }

    #[test]
    fn shell_escape_args_handles_simple_args() {
        let args = vec!["claude".to_string(), "--flag".to_string()];
        assert_eq!(shell_escape_args(&args), "claude --flag");
    }

    #[test]
    fn shell_escape_args_handles_spaces() {
        let args = vec!["echo".to_string(), "hello world".to_string()];
        assert_eq!(shell_escape_args(&args), "echo 'hello world'");
    }
}
