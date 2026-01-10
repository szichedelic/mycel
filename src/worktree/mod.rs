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

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();

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
        std::fs::create_dir_all(parent).context("Failed to create worktree directory")?;
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

    if !config.symlink_paths.is_empty() {
        create_symlinks(git_root, &worktree_path, &config.symlink_paths);
    }

    Ok((worktree_path, branch_name))
}

/// Create a worktree from an existing branch (for unbanking)
pub fn create_from_existing(
    git_root: &Path,
    branch_name: &str,
    config: &ProjectConfig,
) -> Result<(PathBuf, String)> {
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
        std::fs::create_dir_all(parent).context("Failed to create worktree directory")?;
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

    if !config.symlink_paths.is_empty() {
        create_symlinks(git_root, &worktree_path, &config.symlink_paths);
    }

    Ok((worktree_path, branch_name.to_string()))
}

/// Remove a git worktree
pub fn remove(git_root: &Path, worktree_path: &Path) -> Result<()> {
    // Remove the worktree
    let status = Command::new("git")
        .args([
            "worktree",
            "remove",
            "--force",
            &worktree_path.to_string_lossy(),
        ])
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

/// Count commits on a branch relative to a base branch
pub fn commit_count(git_root: &Path, base_branch: &str, branch_name: &str) -> Result<i64> {
    let output = Command::new("git")
        .args([
            "rev-list",
            "--count",
            &format!("{base_branch}..{branch_name}"),
        ])
        .current_dir(git_root)
        .output()
        .context("Failed to count commits")?;

    if !output.status.success() {
        bail!("git rev-list failed");
    }

    let count = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<i64>()
        .unwrap_or(0);

    Ok(count)
}

/// Create symlinks in worktree for configured paths
fn create_symlinks(git_root: &Path, worktree_path: &Path, symlink_paths: &[String]) {
    for pattern in symlink_paths {
        let full_pattern = git_root.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        let matches = match glob::glob(&pattern_str) {
            Ok(paths) => paths,
            Err(e) => {
                eprintln!("Warning: invalid glob pattern '{}': {}", pattern, e);
                continue;
            }
        };

        for entry in matches {
            let source = match entry {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("Warning: glob error for '{}': {}", pattern, e);
                    continue;
                }
            };

            let relative = match source.strip_prefix(git_root) {
                Ok(rel) => rel,
                Err(_) => {
                    eprintln!(
                        "Warning: path '{}' not relative to git root",
                        source.display()
                    );
                    continue;
                }
            };

            let target = worktree_path.join(relative);

            if target.exists() || target.symlink_metadata().is_ok() {
                continue;
            }

            if let Some(parent) = target.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "Warning: failed to create parent dir for '{}': {}",
                        target.display(),
                        e
                    );
                    continue;
                }
            }

            #[cfg(unix)]
            {
                if let Err(e) = std::os::unix::fs::symlink(&source, &target) {
                    eprintln!(
                        "Warning: failed to symlink '{}' -> '{}': {}",
                        source.display(),
                        target.display(),
                        e
                    );
                }
            }

            #[cfg(windows)]
            {
                let result = if source.is_dir() {
                    std::os::windows::fs::symlink_dir(&source, &target)
                } else {
                    std::os::windows::fs::symlink_file(&source, &target)
                };
                if let Err(e) = result {
                    eprintln!(
                        "Warning: failed to symlink '{}' -> '{}': {}",
                        source.display(),
                        target.display(),
                        e
                    );
                }
            }
        }
    }
}
