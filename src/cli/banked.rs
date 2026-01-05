use anyhow::{Context, Result};
use std::env;

use crate::bank;
use crate::db::Database;
use crate::worktree;

pub async fn run() -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let banked = bank::list_banked(&project.name)?;

    if banked.is_empty() {
        println!("No banked sessions for {}.", project.name);
        println!("Bank a session with: mycel bank <session-name>");
        return Ok(());
    }

    println!("Banked sessions for {}:\n", project.name);
    for item in banked {
        println!("  {} ({})", item.name, item.size_human());
    }
    println!("\nRestore with: mycel unbank <name>");

    Ok(())
}
