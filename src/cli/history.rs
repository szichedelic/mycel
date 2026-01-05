use anyhow::{Context, Result};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::Database;
use crate::worktree;

pub async fn run() -> Result<()> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let git_root = worktree::find_git_root(&current_dir)?;

    let db = Database::open()?;

    let project = db
        .get_project_by_path(&git_root)?
        .context("Project not registered. Run 'mycel init' first.")?;

    let history = db.list_session_history(project.id)?;
    if history.is_empty() {
        println!("No session history for {}.", project.name);
        return Ok(());
    }

    let now = current_unix_timestamp();
    println!("Session history for {}:\n", project.name);
    for entry in history {
        let duration_secs = (entry.ended_at_unix - entry.created_at_unix).max(0);
        let duration = format_duration(duration_secs);
        let ended_age = format_relative_age(entry.ended_at_unix, now);
        let commit_text = entry
            .commit_count
            .map(|count| format!("{count} commits"))
            .unwrap_or_else(|| "commits n/a".to_string());

        println!("  {}  {duration}  {commit_text}  ended {ended_age}", entry.name);
        if let Some(note) = entry.note.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            println!("    note: {note}");
        }
    }

    Ok(())
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn format_relative_age(created_at_unix: i64, now_unix: i64) -> String {
    let age_secs = if now_unix > created_at_unix {
        (now_unix - created_at_unix) as u64
    } else {
        0
    };

    if age_secs < 60 {
        "just now".to_string()
    } else if age_secs < 3600 {
        format!("{}m ago", age_secs / 60)
    } else if age_secs < 86_400 {
        format!("{}h ago", age_secs / 3600)
    } else if age_secs < 604_800 {
        format!("{}d ago", age_secs / 86_400)
    } else {
        format!("{}w ago", age_secs / 604_800)
    }
}

fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0) as u64;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86_400 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else {
        format!("{}d {}h", seconds / 86_400, (seconds % 86_400) / 3600)
    }
}
