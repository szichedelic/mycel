use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::Database;
use crate::session::SessionManager;

pub async fn run(idle_minutes: u64, dry_run: bool) -> Result<()> {
    let db = Database::open()?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let cutoff = now - (idle_minutes as i64 * 60);

    let idle = db.find_idle_runtimes(cutoff)?;

    if idle.is_empty() {
        println!("No idle runtimes found (threshold: {idle_minutes}m).");
        return Ok(());
    }

    println!(
        "Found {} idle runtime(s) (not seen in {idle_minutes}m):\n",
        idle.len()
    );

    for (session, runtime) in &idle {
        println!(
            "  {} [{}] on {} (last seen {}s ago)",
            session.name,
            runtime.provider,
            runtime.host,
            now - runtime.last_seen_unix
        );
    }

    if dry_run {
        println!("\nDry run — no action taken. Remove --dry-run to reap.");
        return Ok(());
    }

    println!();
    let mut reaped = 0;
    for (session, runtime) in &idle {
        let sm = SessionManager::for_kind_str(&session.runtime_kind);
        if sm.is_alive(&session.tmux_session).unwrap_or(false) {
            if let Err(e) = sm.kill(&session.tmux_session) {
                eprintln!("  Failed to kill {}: {e}", session.name);
                continue;
            }
        }
        if let Err(e) = db.update_runtime_state(runtime.id, "reaped") {
            eprintln!("  Failed to update state for {}: {e}", session.name);
            continue;
        }
        println!("  Reaped: {}", session.name);
        reaped += 1;
    }

    println!("\nReaped {reaped}/{} idle runtime(s).", idle.len());
    Ok(())
}
