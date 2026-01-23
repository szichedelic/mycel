use anyhow::{bail, Context, Result};
use std::env;
use std::time::Duration;

use crate::config::{resolve_backend, GlobalConfig, ProjectConfig, TemplateConfig};
use crate::db::{Database, NewSession};
use crate::session::SessionManager;
use crate::worktree;

pub async fn run(
    name: &str,
    note: Option<&str>,
    template_name: Option<&str>,
    backend_override: Option<&str>,
    happy: bool,
) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let config = ProjectConfig::load(&git_root)?;
    let global_config = GlobalConfig::load()?;
    let mut backend = resolve_backend(&global_config, &config, backend_override)?;

    if happy {
        apply_happy_wrapper(&mut backend)?;
    }
    let template = match template_name {
        Some(name) => Some(
            config
                .templates
                .get(name)
                .with_context(|| format!("Template '{name}' not found in config"))?,
        ),
        None => None,
    };

    if db.get_session_by_name(project.id, name)?.is_some() {
        bail!("Session '{name}' already exists in this project");
    }

    println!("Creating worktree...");
    let (worktree_path, session_id) = worktree::create(&git_root, &config)?;

    // Get the branch name that was created
    let branch_name = worktree::get_branch(&worktree_path)?;

    println!("Starting {} session...", backend.name);
    let setup = merge_setup(&config, template);
    if !setup.is_empty() {
        println!("Setup: {}", setup.join(" && "));
    }
    let session_manager = SessionManager::new();
    let tmux_session =
        session_manager.create(&project.name, &session_id, &worktree_path, &setup, &backend)?;

    db.add_session(&NewSession {
        project_id: project.id,
        name,
        branch_name: &branch_name,
        worktree_path: &worktree_path,
        tmux_session: &tmux_session,
        backend: &backend.name,
        note,
    })?;

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

    println!("\nSession '{name}' created. Attach with: mycel attach {name}");

    Ok(())
}

fn merge_setup(config: &ProjectConfig, template: Option<&TemplateConfig>) -> Vec<String> {
    let mut setup = config.setup.clone();
    if let Some(template) = template {
        setup.extend(template.setup.clone());
    }
    setup
}

use crate::config::ResolvedBackend;

fn apply_happy_wrapper(backend: &mut ResolvedBackend) -> Result<()> {
    if !is_happy_available() {
        bail!(
            "Happy CLI not found. Install with: npm install -g happy-coder\n\
             Learn more: https://github.com/slopus/happy-cli"
        );
    }

    let original_command = std::mem::replace(&mut backend.command, "happy".to_string());
    let original_args = std::mem::take(&mut backend.args);

    backend.args = vec![original_command];
    backend.args.extend(original_args);

    Ok(())
}

fn is_happy_available() -> bool {
    std::process::Command::new("which")
        .arg("happy")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
