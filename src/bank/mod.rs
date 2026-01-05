use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::ProjectConfig;

/// Get the bank directory for a project
pub fn bank_dir(project_name: &str) -> Result<PathBuf> {
    let base = dirs::home_dir()
        .context("Could not find home directory")?
        .join(".mycel")
        .join("bank")
        .join(project_name);

    fs::create_dir_all(&base)?;
    Ok(base)
}

/// Get the bundle path for a banked session
pub fn bundle_path(project_name: &str, session_name: &str) -> Result<PathBuf> {
    Ok(bank_dir(project_name)?.join(format!("{}.bundle", session_name)))
}

/// Create a git bundle from a branch
pub fn create_bundle(
    git_root: &Path,
    branch_name: &str,
    base_branch: &str,
    bundle_path: &Path,
) -> Result<()> {
    // Check if there are commits to bundle
    let output = Command::new("git")
        .args(["rev-list", "--count", &format!("{}..{}", base_branch, branch_name)])
        .current_dir(git_root)
        .output()
        .context("Failed to check commit count")?;

    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);

    if count == 0 {
        bail!("No commits to bank. Did you forget to commit your work?");
    }

    // Create the bundle
    let status = Command::new("git")
        .args([
            "bundle",
            "create",
            &bundle_path.to_string_lossy(),
            &format!("{}..{}", base_branch, branch_name),
        ])
        .current_dir(git_root)
        .status()
        .context("Failed to create bundle")?;

    if !status.success() {
        bail!("git bundle create failed");
    }

    println!("Banked {} commits", count);
    Ok(())
}

/// Verify a bundle is valid
pub fn verify_bundle(bundle_path: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["bundle", "verify", &bundle_path.to_string_lossy()])
        .status()
        .context("Failed to verify bundle")?;

    if !status.success() {
        bail!("Bundle verification failed");
    }

    Ok(())
}

/// Restore a branch from a bundle
pub fn restore_bundle(git_root: &Path, bundle_path: &Path, branch_name: &str) -> Result<()> {
    // Fetch the branch from the bundle
    let status = Command::new("git")
        .args([
            "fetch",
            &bundle_path.to_string_lossy(),
            &format!("{}:{}", branch_name, branch_name),
        ])
        .current_dir(git_root)
        .status()
        .context("Failed to restore from bundle")?;

    if !status.success() {
        bail!("git fetch from bundle failed");
    }

    Ok(())
}

/// List all banked bundles for a project
pub fn list_banked(project_name: &str) -> Result<Vec<BankedItem>> {
    let dir = bank_dir(project_name)?;
    let mut items = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "bundle").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let metadata = fs::metadata(&path)?;
                let size = metadata.len();

                items.push(BankedItem {
                    name,
                    path,
                    size,
                });
            }
        }
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// Delete a bundle file
pub fn delete_bundle(bundle_path: &Path) -> Result<()> {
    fs::remove_file(bundle_path).context("Failed to delete bundle")?;
    Ok(())
}

#[derive(Debug)]
pub struct BankedItem {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
}

impl BankedItem {
    pub fn size_human(&self) -> String {
        if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{:.1} KB", self.size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        }
    }
}
