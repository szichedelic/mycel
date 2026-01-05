use anyhow::{bail, Context, Result};
use std::env;
use std::process::Command;
use std::time::Duration;

use crate::config::{ProjectConfig, TemplateConfig};
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

pub async fn run(name: &str, note: Option<&str>, template_name: Option<&str>) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let config = ProjectConfig::load(&git_root)?;
    let template = match template_name {
        Some(name) => Some(
            config
                .templates
                .get(name)
                .with_context(|| format!("Template '{name}' not found in .mycel.toml"))?,
        ),
        None => None,
    };
    let full_name = apply_template_prefix(name, template.and_then(|t| t.branch_prefix.as_deref()));
    let sanitized_name = worktree::sanitize_branch_name(&full_name);

    if db
        .get_session_by_name(project.id, &sanitized_name)?
        .is_some()
    {
        bail!("Session '{sanitized_name}' already exists in this project");
    }

    let (worktree_path, branch_name) = if branch_exists(&git_root, &sanitized_name) {
        println!("Using existing branch '{sanitized_name}'...");
        worktree::create_from_existing(&git_root, &sanitized_name, &config)?
    } else {
        println!("Creating worktree '{sanitized_name}'...");
        worktree::create(&git_root, &full_name, &config)?
    };

    println!("Starting Claude session...");
    let setup = merge_setup(&config, template);
    if !setup.is_empty() {
        println!("Setup: {}", setup.join(" && "));
    }
    let session_manager = SessionManager::new();
    let tmux_session =
        session_manager.create(&project.name, &branch_name, &worktree_path, &setup)?;

    db.add_session(
        project.id,
        &branch_name,
        &worktree_path,
        &tmux_session,
        note,
    )?;

    if let Some(prompt) = template
        .and_then(|t| t.prompt.as_deref())
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        std::thread::sleep(Duration::from_millis(600));
        if let Err(err) = session_manager.send_prompt(&tmux_session, prompt) {
            eprintln!("Warning: failed to send template prompt: {err}");
        }
    }

    println!("\nSession '{branch_name}' created. Attach with: mycel attach {branch_name}");

    Ok(())
}

fn apply_template_prefix(name: &str, prefix: Option<&str>) -> String {
    match prefix {
        Some(prefix) if !prefix.is_empty() && !name.starts_with(prefix) => {
            format!("{prefix}{name}")
        }
        _ => name.to_string(),
    }
}

fn merge_setup(config: &ProjectConfig, template: Option<&TemplateConfig>) -> Vec<String> {
    let mut setup = config.setup.clone();
    if let Some(template) = template {
        setup.extend(template.setup.clone());
    }
    setup
}
