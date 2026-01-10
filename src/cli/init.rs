use anyhow::{Context, Result};
use std::env;

use crate::config::{get_external_config_path, ProjectConfig};
use crate::confirm::{prompt_confirm, prompt_multi, prompt_select, prompt_string};
use crate::db::Database;
use crate::worktree;

pub async fn run(skip_wizard: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let name = git_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();

    let already_registered = db.get_project_by_path(&git_root)?.is_some();
    let config_exists = ProjectConfig::exists(&git_root);

    if already_registered && config_exists && !skip_wizard {
        println!("Project already set up: {}", git_root.display());
        if !prompt_confirm("Reconfigure?")? {
            return Ok(());
        }
    }

    if skip_wizard {
        if !already_registered {
            db.add_project(&name, &git_root)?;
            println!("Registered project: {} ({})", name, git_root.display());
        } else {
            println!("Project already registered: {}", git_root.display());
        }
        return Ok(());
    }

    println!("\n  Welcome to mycel setup wizard\n");

    // Base branch
    let base_branch = prompt_string("Base branch", "main")?;

    // Worktree directory
    let worktree_dir = prompt_string("Worktree directory", "../.mycel-worktrees")?;

    // Backend
    let backend_options = ["claude", "codex", "other"];
    let backend_idx = prompt_select("Default backend:", &backend_options, 0)?;
    let backend = if backend_idx == 2 {
        Some(prompt_string("Backend name", "")?)
    } else if backend_idx == 0 {
        None // claude is the default, no need to set explicitly
    } else {
        Some(backend_options[backend_idx].to_string())
    };

    // Symlink paths
    let symlink_options = [".claude", ".env.local", ".vscode", ".idea"];
    let symlink_defaults: Vec<usize> = vec![]; // No defaults selected
    let symlink_indices = prompt_multi("Symlink paths to worktrees:", &symlink_options, &symlink_defaults)?;
    let symlink_paths: Vec<String> = symlink_indices
        .iter()
        .map(|&i| symlink_options[i].to_string())
        .collect();

    // Config location
    let location_options = [
        &format!("External (~/.config/mycel/projects/{name}.toml)"),
        "In-repo (.mycel.toml)",
    ];
    let location_idx = prompt_select("Save config to:", &location_options, 0)?;

    // Build config
    let config = ProjectConfig {
        base_branch,
        worktree_dir,
        backend,
        symlink_paths,
        ..Default::default()
    };

    // Save config
    let config_path = if location_idx == 0 {
        get_external_config_path(&name).context("Could not determine config path")?
    } else {
        git_root.join(".mycel.toml")
    };

    config.save(&config_path)?;
    println!("\nConfig saved to: {}", config_path.display());

    // Register project
    if !already_registered {
        db.add_project(&name, &git_root)?;
    }

    println!("Project ready: {} ({})\n", name, git_root.display());
    println!("Next steps:");
    println!("  mycel spawn <name>  - Create a new session");
    println!("  mycel               - Open TUI dashboard");

    Ok(())
}
