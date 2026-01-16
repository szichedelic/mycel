use anyhow::{bail, Context, Result};
use std::env;

use crate::db::Database;
use crate::worktree;

pub async fn run(from: &str, to: &str) -> Result<()> {
    let from = from.trim();
    let to = to.trim();

    if from.is_empty() {
        bail!("Current session name cannot be empty");
    }

    if to.is_empty() {
        bail!("New session name cannot be empty");
    }

    if from == to {
        bail!("New session name matches existing name");
    }

    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let session = db
        .get_session_by_name(project.id, from)?
        .context(format!("Session '{from}' not found"))?;

    if db.get_session_by_name(project.id, to)?.is_some() {
        bail!("Session '{to}' already exists in this project");
    }

    db.update_session_name(session.id, to)?;

    println!("Renamed session '{from}' to '{to}'.");

    Ok(())
}
