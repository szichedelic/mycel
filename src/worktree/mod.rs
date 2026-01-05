use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::ProjectConfig;

/// Find the git repository root from a given path
pub fn find_git_root(from: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(from)
        .output()
        .context("Failed to run git rev-parse")?;

    if !output.status.success() {
        bail!("Not a git repository");
    }

    let path = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    Ok(PathBuf::from(path))
}

/// Sanitize a name to be a valid git branch name
pub fn sanitize_branch_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            ' ' | '\t' | '~' | '^' | ':' | '?' | '*' | '[' | '\\' => '-',
            c => c,
        })
        .collect::<String>()
        .replace("..", "-")
        .replace("@{", "-")
        .trim_matches(|c| c == '.' || c == '/' || c == '-')
        .to_string()
}

/// Create a new git worktree. Returns (worktree_path, sanitized_branch_name)
pub fn create(git_root: &Path, name: &str, config: &ProjectConfig) -> Result<(PathBuf, String)> {
    // Sanitize name for git branch
    let branch_name = sanitize_branch_name(name);

    // Resolve worktree directory
    let worktree_base = if config.worktree_dir.starts_with('/') {
        PathBuf::from(&config.worktree_dir)
    } else {
        git_root.join(&config.worktree_dir)
    };

    // Get project name for namespacing
    let project_name = git_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let worktree_path = worktree_base.join(project_name).join(&branch_name);

    // Ensure parent directory exists
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create worktree directory")?;
    }

    // Create the worktree
    let status = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &branch_name,
            &worktree_path.to_string_lossy(),
            &config.base_branch,
        ])
        .current_dir(git_root)
        .status()
        .context("Failed to create git worktree")?;

    if !status.success() {
        bail!("git worktree add failed");
    }

    Ok((worktree_path, branch_name))
}

/// Create a worktree from an existing branch (for unbanking)
pub fn create_from_existing(git_root: &Path, branch_name: &str, config: &ProjectConfig) -> Result<(PathBuf, String)> {
    // Resolve worktree directory
    let worktree_base = if config.worktree_dir.starts_with('/') {
        PathBuf::from(&config.worktree_dir)
    } else {
        git_root.join(&config.worktree_dir)
    };

    let project_name = git_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let worktree_path = worktree_base.join(project_name).join(branch_name);

    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create worktree directory")?;
    }

    // Create worktree from existing branch (no -b flag)
    let status = Command::new("git")
        .args([
            "worktree",
            "add",
            &worktree_path.to_string_lossy(),
            branch_name,
        ])
        .current_dir(git_root)
        .status()
        .context("Failed to create git worktree")?;

    if !status.success() {
        bail!("git worktree add failed");
    }

    Ok((worktree_path, branch_name.to_string()))
}

/// Run setup commands in a worktree
pub fn run_setup(worktree_path: &Path, commands: &[String]) -> Result<()> {
    for cmd in commands {
        println!("  Running: {}", cmd);

        let status = Command::new("sh")
            .args(["-c", cmd])
            .current_dir(worktree_path)
            .status()
            .context(format!("Failed to run setup command: {}", cmd))?;

        if !status.success() {
            bail!("Setup command failed: {}", cmd);
        }
    }

    Ok(())
}

/// Remove a git worktree
pub fn remove(git_root: &Path, worktree_path: &Path) -> Result<()> {
    // Remove the worktree
    let status = Command::new("git")
        .args(["worktree", "remove", "--force", &worktree_path.to_string_lossy()])
        .current_dir(git_root)
        .status()
        .context("Failed to remove git worktree")?;

    if !status.success() {
        bail!("git worktree remove failed");
    }

    // Also delete the branch
    let branch_name = worktree_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if !branch_name.is_empty() {
        // Ignore errors here - branch might not exist
        let _ = Command::new("git")
            .args(["branch", "-D", branch_name])
            .current_dir(git_root)
            .status();
    }

    Ok(())
}

/// List existing worktrees for a repository
pub fn list(git_root: &Path) -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(git_root)
        .output()
        .context("Failed to list git worktrees")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(path));
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = Some(branch.to_string());
        } else if line.is_empty() {
            if let (Some(path), Some(branch)) = (current_path.take(), current_branch.take()) {
                worktrees.push(WorktreeInfo { path, branch });
            }
            current_path = None;
            current_branch = None;
        }
    }

    // Handle last entry if no trailing newline
    if let (Some(path), Some(branch)) = (current_path, current_branch) {
        worktrees.push(WorktreeInfo { path, branch });
    }

    Ok(worktrees)
}

#[derive(Debug)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
}
