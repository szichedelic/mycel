use anyhow::{Context, Result};
use std::env;

use crate::confirm;
use crate::db::Database;
use crate::session::SessionManager;
use crate::worktree;

pub async fn run(name: &str, force: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let session = db
        .get_session_by_name(project.id, name)?
        .context(format!("Session '{name}' not found"))?;

    if !force {
        let prompt = format!("Stop session '{name}'?");
        if !confirm::prompt_confirm(&prompt)? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let session_manager = SessionManager::for_kind_str(&session.runtime_kind);

    if session_manager.is_alive(&session.tmux_session)? {
        println!("Stopping session '{name}'...");
        session_manager.kill(&session.tmux_session)?;
    } else {
        println!("Session '{name}' already stopped.");
    }

    println!("Worktree preserved at: {}", session.worktree_path.display());

    Ok(())
}
