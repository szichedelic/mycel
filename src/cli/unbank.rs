use anyhow::{bail, Context, Result};
use std::env;

use crate::bank;
use crate::config::{resolve_backend, GlobalConfig, ProjectConfig};
use crate::confirm;
use crate::db::{Database, NewSession};
use crate::session::SessionManager;
use crate::worktree;

pub async fn run(name: &str, spawn: bool, force: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let bundle_path = bank::bundle_path(&project.name, name)?;

    if !bundle_path.exists() {
        bail!("No banked bundle found for '{name}'. List banked with: mycel banked");
    }

    println!("Verifying bundle...");
    bank::verify_bundle(&bundle_path)?;

    if !force {
        let prompt =
            format!("Unbanking '{name}' will restore the branch and delete the bundle. Continue?");
        if !confirm::prompt_confirm(&prompt)? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let metadata = bank::read_metadata(&project.name, name)?;
    let display_name = metadata
        .as_ref()
        .map(|m| m.session_name.clone())
        .unwrap_or_else(|| name.to_string());
    let restored_note = metadata.as_ref().and_then(|m| m.note.clone());

    println!("Restoring branch '{name}'...");
    bank::restore_bundle(&git_root, &bundle_path, name)?;

    bank::delete_bundle(&bundle_path)?;
    bank::delete_metadata(&project.name, name)?;
    println!("Bundle removed from bank.");

    if spawn {
        let config = ProjectConfig::load(&git_root)?;
        let global_config = GlobalConfig::load()?;
        let backend = resolve_backend(&global_config, &config, None)?;

        println!("Creating worktree...");
        let (worktree_path, session_id) = worktree::create_from_existing(&git_root, name, &config)?;

        println!("Starting {} session...", backend.name);
        let session_manager = SessionManager::new();
        let tmux_session = session_manager.create(
            &project.name,
            &session_id,
            &worktree_path,
            &config.setup,
            &backend,
        )?;

        db.add_session(&NewSession {
            project_id: project.id,
            name: &display_name,
            branch_name: name,
            worktree_path: &worktree_path,
            tmux_session: &tmux_session,
            runtime_kind: session_manager.kind().as_str(),
            backend: &backend.name,
            note: restored_note.as_deref(),
        })?;

        println!("\nSession '{display_name}' restored. Attach with: mycel attach {display_name}");
    } else {
        println!("\nBranch '{name}' restored. Create a session with: mycel spawn <session-name>");
    }

    Ok(())
}
