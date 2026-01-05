use anyhow::{bail, Context, Result};
use std::env;

use crate::bank;
use crate::config::ProjectConfig;
use crate::confirm;
use crate::db::Database;
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

    println!("Restoring branch '{name}'...");
    bank::restore_bundle(&git_root, &bundle_path, name)?;

    bank::delete_bundle(&bundle_path)?;
    bank::delete_metadata(&project.name, name)?;
    println!("Bundle removed from bank.");

    if spawn {
        let config = ProjectConfig::load(&git_root)?;

        println!("Creating worktree...");
        let (worktree_path, branch_name) =
            worktree::create_from_existing(&git_root, name, &config)?;

        println!("Starting Claude session...");
        let session_manager = SessionManager::new();
        let tmux_session =
            session_manager.create(&project.name, &branch_name, &worktree_path, &config.setup)?;

        db.add_session(
            project.id,
            &branch_name,
            &worktree_path,
            &tmux_session,
            None,
        )?;

        println!("\nSession '{name}' restored. Attach with: mycel attach {name}");
    } else {
        println!("\nBranch '{name}' restored. Create a session with: mycel spawn {name}");
    }

    Ok(())
}
