use anyhow::{Context, Result};
use std::env;

use crate::db::Database;
use crate::session::SessionManager;
use crate::worktree;

pub async fn run(name: &str, remove_worktree: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let session = db
        .get_session_by_name(project.id, name)?
        .context(format!("Session '{}' not found", name))?;

    let session_manager = SessionManager::new();

    // Kill tmux session if running
    if session_manager.is_alive(&session.tmux_session)? {
        println!("Stopping session '{}'...", name);
        session_manager.kill(&session.tmux_session)?;
    }

    // Remove worktree if requested
    if remove_worktree {
        println!("Removing worktree...");
        worktree::remove(&git_root, &session.worktree_path)?;
    }

    // Remove from database
    db.delete_session(session.id)?;

    println!("Session '{}' killed.", name);
    if !remove_worktree {
        println!("Worktree preserved at: {}", session.worktree_path.display());
    }

    Ok(())
}
