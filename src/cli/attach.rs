use anyhow::{Context, Result};
use std::env;

use crate::config::{resolve_backend, GlobalConfig, ProjectConfig};
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
        .context(format!("Session '{name}' not found"))?;

    let session_manager = SessionManager::for_kind_str(&session.runtime_kind);

    let tmux_session = if !session_manager.is_alive(&session.tmux_session)? {
        println!("Session '{name}' is not running. Restarting...");
        let _ = session_manager.kill(&session.tmux_session);
        let config = ProjectConfig::load(&git_root)?;
        let global_config = GlobalConfig::load()?;
        let backend = resolve_backend(&global_config, &config, Some(&session.backend))?;
        let new_tmux = session_manager.create(
            &project.name,
            name,
            &session.worktree_path,
            &config.setup,
            &backend,
        )?;
        if new_tmux != session.tmux_session {
            db.update_session_tmux(session.id, &new_tmux)?;
        }
        new_tmux
    } else {
        session.tmux_session.clone()
    };

    if let Err(err) = session_manager.set_session_label(&tmux_session, &project.name, name) {
        eprintln!("Warning: failed to set session label: {err}");
    }

    session_manager.attach(&tmux_session)?;

    Ok(())
}
