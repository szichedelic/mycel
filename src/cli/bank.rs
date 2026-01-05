use anyhow::{bail, Context, Result};
use std::env;

use crate::bank;
use crate::config::ProjectConfig;
use crate::db::Database;
use crate::session::SessionManager;
use crate::worktree;

pub async fn run(name: &str, keep: bool) -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let session = db
        .get_session_by_name(project.id, name)?
        .context(format!("Session '{}' not found", name))?;

    let config = ProjectConfig::load(&git_root)?;

    let status_output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&session.worktree_path)
        .output()
        .context("Failed to check git status")?;

    let has_changes = !status_output.stdout.is_empty();
    if has_changes {
        bail!(
            "Uncommitted changes in worktree. Commit your work first:\n  cd {}\n  git add -A && git commit -m \"your message\"",
            session.worktree_path.display()
        );
    }

    let bundle_path = bank::bundle_path(&project.name, name)?;

    if bundle_path.exists() {
        bail!("Bundle already exists: {}. Delete it first or use a different name.", bundle_path.display());
    }

    println!("Banking '{}'...", name);
    bank::create_bundle(&git_root, name, &config.base_branch, &bundle_path)?;
    println!("Bundle saved: {}", bundle_path.display());

    // Kill session and remove worktree unless --keep
    if !keep {
        let session_manager = SessionManager::new();

        if session_manager.is_alive(&session.tmux_session)? {
            println!("Stopping session...");
            session_manager.kill(&session.tmux_session)?;
        }

        println!("Removing worktree...");
        worktree::remove(&git_root, &session.worktree_path)?;

        // Remove from database
        db.delete_session(session.id)?;

        // Also delete the local branch since it's in the bundle now
        let _ = std::process::Command::new("git")
            .args(["branch", "-D", name])
            .current_dir(&git_root)
            .status();
    }

    println!("\nBanked '{}'. Restore with: mycel unbank {}", name, name);

    Ok(())
}
