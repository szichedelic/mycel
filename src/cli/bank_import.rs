use anyhow::{Context, Result};
use std::env;
use std::path::Path;

use crate::bank;
use crate::db::Database;
use crate::worktree;

pub async fn run(path: &Path, name: Option<&str>, force: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let imported = bank::import_bundle(path, &project.name, name, force)?;
    println!(
        "Imported bank '{}' to {}",
        imported.session_name,
        imported.bundle_path.display()
    );

    Ok(())
}
