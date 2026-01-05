use anyhow::{bail, Context, Result};
use std::env;

use crate::config::ProjectConfig;
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

    let sanitized_name = worktree::sanitize_branch_name(name);

    if db.get_session_by_name(project.id, &sanitized_name)?.is_some() {
        bail!("Session '{}' already exists in this project", sanitized_name);
    }

    let config = ProjectConfig::load(&git_root)?;

    println!("Creating worktree '{}'...", sanitized_name);
    let (worktree_path, branch_name) = worktree::create(&git_root, name, &config)?;

    // Setup commands run in the tmux shell
    println!("Starting Claude session...");
    if !config.setup.is_empty() {
        println!("Setup: {}", config.setup.join(" && "));
    }
    let session_manager = SessionManager::new();
    let tmux_session = session_manager.create(&project.name, &branch_name, &worktree_path, &config.setup)?;

    db.add_session(project.id, &branch_name, &worktree_path, &tmux_session)?;

    println!(
        "\nSession '{}' created. Attach with: mycel attach {}",
        branch_name, branch_name
    );

    Ok(())
}
