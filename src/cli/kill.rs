use anyhow::{Context, Result};
use std::env;

use crate::config::ProjectConfig;
use crate::confirm;
use crate::db::Database;
use crate::session::SessionManager;
use crate::worktree;

pub async fn run(name: &str, remove_worktree: bool, force: bool) -> Result<()> {
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
        let prompt = if remove_worktree {
            format!("Kill session '{name}' and remove its worktree?")
        } else {
            format!("Kill session '{name}'?")
        };

        if !confirm::prompt_confirm(&prompt)? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let session_manager = SessionManager::new();

    // Kill tmux session if running
    if session_manager.is_alive(&session.tmux_session)? {
        println!("Stopping session '{name}'...");
        session_manager.kill(&session.tmux_session)?;
    }

    let config = ProjectConfig::load(&git_root)?;
    let commit_count = worktree::commit_count(&git_root, &config.base_branch, &session.name).ok();

    // Remove worktree if requested
    if remove_worktree {
        println!("Removing worktree...");
        worktree::remove(&git_root, &session.worktree_path)?;
    }

    db.archive_session(project.id, &session, commit_count)?;

    // Remove from database
    db.delete_session(session.id)?;

    println!("Session '{name}' killed.");
    if !remove_worktree {
        println!("Worktree preserved at: {}", session.worktree_path.display());
    }

    Ok(())
}
