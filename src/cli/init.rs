use anyhow::{Context, Result};
use std::env;

use crate::db::Database;
use crate::worktree;

pub async fn run() -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;

    // Verify this is a git repository
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    // Check if already registered
    if db.get_project_by_path(&git_root)?.is_some() {
        println!("Project already registered: {}", git_root.display());
        return Ok(());
    }

    // Extract project name from directory
    let name = git_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();

    db.add_project(&name, &git_root)?;

    println!("Registered project: {} ({})", name, git_root.display());
    Ok(())
}
