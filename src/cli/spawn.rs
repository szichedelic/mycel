use anyhow::{bail, Context, Result};
use std::env;
use std::process::Command;

use crate::config::ProjectConfig;
use crate::db::Database;
use crate::session::SessionManager;
use crate::worktree;

fn branch_exists(git_root: &std::path::Path, branch_name: &str) -> bool {
    Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch_name}"),
        ])
        .current_dir(git_root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub async fn run(name: &str) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let sanitized_name = worktree::sanitize_branch_name(name);

    if db
        .get_session_by_name(project.id, &sanitized_name)?
        .is_some()
    {
        bail!("Session '{sanitized_name}' already exists in this project");
    }

    let config = ProjectConfig::load(&git_root)?;

    let (worktree_path, branch_name) = if branch_exists(&git_root, &sanitized_name) {
        println!("Using existing branch '{sanitized_name}'...");
        worktree::create_from_existing(&git_root, &sanitized_name, &config)?
    } else {
        println!("Creating worktree '{sanitized_name}'...");
        worktree::create(&git_root, name, &config)?
    };

    println!("Starting Claude session...");
    if !config.setup.is_empty() {
        println!("Setup: {}", config.setup.join(" && "));
    }
    let session_manager = SessionManager::new();
    let tmux_session =
        session_manager.create(&project.name, &branch_name, &worktree_path, &config.setup)?;

    db.add_session(project.id, &branch_name, &worktree_path, &tmux_session)?;

    println!("\nSession '{branch_name}' created. Attach with: mycel attach {branch_name}");

    Ok(())
}
