use anyhow::{Context, Result};
use std::env;

use crate::db::Database;
use crate::session::SessionManager;
use crate::worktree;

pub async fn run(name: &str) -> Result<()> {
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

    // Check if session is still alive
    if !session_manager.is_alive(&session.tmux_session)? {
        println!("Session '{}' is not running. Restarting...", name);
        session_manager.create(&project.name, name, &session.worktree_path)?;
    }

    // Attach to the session
    session_manager.attach(&session.tmux_session)?;

    Ok(())
}
