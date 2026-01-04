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

    // Ensure project is registered
    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    // Check if session with this name already exists
    if db.get_session_by_name(project.id, name)?.is_some() {
        bail!("Session '{}' already exists in this project", name);
    }

    // Load project config
    let config = ProjectConfig::load(&git_root)?;

    // Create worktree
    println!("Creating worktree '{}'...", name);
    let worktree_path = worktree::create(&git_root, name, &config)?;

    // Run setup commands
    if !config.setup.is_empty() {
        println!("Running setup commands...");
        worktree::run_setup(&worktree_path, &config.setup)?;
    }

    // Start tmux session with claude
    println!("Starting Claude session...");
    let session_manager = SessionManager::new();
    let tmux_session = session_manager.create(&project.name, name, &worktree_path)?;

    // Save to database
    db.add_session(project.id, name, &worktree_path, &tmux_session)?;

    println!(
        "\nSession '{}' created. Attach with: mycel attach {}",
        name, name
    );

    Ok(())
}
