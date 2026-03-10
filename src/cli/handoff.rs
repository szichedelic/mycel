use anyhow::{bail, Context, Result};
use std::env;

use crate::config::{resolve_backend, GlobalConfig, ProjectConfig};
use crate::confirm;
use crate::db::Database;
use crate::session::runtime::RuntimeKind;
use crate::session::{handoff_session, HandoffTarget};
use crate::worktree;

pub async fn run(name: &str, to: &str, host: Option<&str>, force: bool) -> Result<()> {
    let dest_kind = RuntimeKind::from_str(to)
        .with_context(|| format!("Unknown runtime kind '{to}'. Use: tmux, compose, remote"))?;

    if dest_kind == RuntimeKind::Remote && host.is_none() {
        bail!("--host is required for remote handoff (e.g. --host ssh://user@host)");
    }

    let dest_host = match dest_kind {
        RuntimeKind::Remote => host.unwrap().to_string(),
        _ => "local".to_string(),
    };

    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let session = db
        .get_session_by_name(project.id, name)?
        .context(format!("Session '{name}' not found"))?;

    let source_kind = &session.runtime_kind;
    if source_kind == dest_kind.as_str() && dest_host == "local" {
        bail!("Session '{name}' is already running on {source_kind}");
    }

    if !force {
        let prompt = format!(
            "Hand off session '{name}' from {source_kind} to {to}{}?",
            if dest_kind == RuntimeKind::Remote {
                format!(" ({dest_host})")
            } else {
                String::new()
            }
        );
        if !confirm::prompt_confirm(&prompt)? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let config = ProjectConfig::load(&git_root)?;
    let global_config = GlobalConfig::load()?;
    let backend = resolve_backend(&global_config, &config, Some(&session.backend))?;

    let target = HandoffTarget {
        kind: dest_kind,
        host: dest_host,
    };

    println!(
        "Handing off '{name}' from {} to {}...",
        source_kind,
        dest_kind.as_str()
    );

    let result = handoff_session(
        &db,
        &session,
        &project.name,
        &target,
        &backend,
        &config.setup,
    )?;

    println!(
        "Session '{name}' is now running on {} (runtime: {})",
        result.new_kind, result.new_runtime_id
    );

    Ok(())
}
