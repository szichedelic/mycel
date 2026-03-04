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

    let mut config = if config_exists {
        ProjectConfig::load(&git_root)?
    } else {
        ProjectConfig::default()
    };

    // Base branch
    let base_branch = prompt_string("Base branch", &config.base_branch)?;

    // Worktree directory
    let worktree_dir = prompt_string("Worktree directory", &config.worktree_dir)?;

    // Backend
    let backend_options = ["claude", "codex", "other"];
    let backend_default = match config.backend.as_deref() {
        Some("codex") => 1,
        Some("claude") | None => 0,
        Some(_) => 2,
    };
    let backend_idx = prompt_select("Default backend:", &backend_options, backend_default)?;
    let backend = if backend_idx == 2 {
        let default_backend = config
            .backend
            .as_deref()
            .filter(|name| *name != "claude" && *name != "codex")
            .unwrap_or("");
        let backend_name = prompt_string("Backend name", default_backend)?;
        if backend_name.is_empty() {
            None
        } else {
            Some(backend_name)
        }
    } else if backend_idx == 0 {
        None // claude is the default, no need to set explicitly
    } else {
        Some(backend_options[backend_idx].to_string())
    };

    // Symlink paths
    let symlink_options = [".claude", ".env.local", ".vscode", ".idea"];
    let symlink_defaults: Vec<usize> = symlink_options
        .iter()
        .enumerate()
        .filter_map(|(idx, option)| {
            if config.symlink_paths.iter().any(|path| path == option) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();
    let symlink_indices = prompt_multi(
        "Symlink paths to worktrees:",
        &symlink_options,
        &symlink_defaults,
    )?;
    let mut symlink_paths: Vec<String> = Vec::new();
    for idx in symlink_indices {
        let path = symlink_options[idx].to_string();
        if !symlink_paths.contains(&path) {
            symlink_paths.push(path);
        }
    }
    for path in &config.symlink_paths {
        if !symlink_options.iter().any(|option| option == path) && !symlink_paths.contains(path) {
            symlink_paths.push(path.clone());
        }
    }

    config.base_branch = base_branch;
    config.worktree_dir = worktree_dir;
    config.backend = backend;
    config.symlink_paths = symlink_paths;

    // Save config
    let external_config_path =
        get_external_config_path(&name).context("Could not determine config path")?;
    let repo_config_path = git_root.join(".mycel.toml");
    let config_path = if external_config_path.exists() {
        external_config_path.clone()
    } else if repo_config_path.exists() {
        repo_config_path.clone()
    } else {
        let location_options = [
            &format!("External (~/.config/mycel/projects/{name}.toml)"),
            "In-repo (.mycel.toml)",
        ];
        let location_idx = prompt_select("Save config to:", &location_options, 0)?;
        if location_idx == 0 {
            external_config_path
        } else {
            repo_config_path
        }
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
