use anyhow::{Context, Result};
use std::env;

use crate::db::Database;
use crate::session::SessionManager;
use crate::worktree;

pub async fn run() -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let sessions = db.list_sessions(project.id)?;

    if sessions.is_empty() {
        println!("No sessions in this project. Create one with: mycel spawn <name>");
        return Ok(());
    }

    let session_manager = SessionManager::new();

    println!("Sessions in {}:\n", project.name);
    for session in sessions {
        let status = if session_manager.is_alive(&session.tmux_session)? {
            "running"
        } else {
            "stopped"
        };
        println!(
            "  {} [{}] ({}) - {}",
            session.name,
            status,
            session.backend,
            session.worktree_path.display()
        );
    }

    Ok(())
}
