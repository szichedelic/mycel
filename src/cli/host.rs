use anyhow::{Context, Result};

use crate::db::Database;

pub async fn add(name: &str, docker_host: &str, max_sessions: i64) -> Result<()> {
    let db = Database::open()?;

    db.add_host(name, docker_host, max_sessions)
        .context(format!("Failed to add host '{name}' (already exists?)"))?;

    println!("Host '{name}' added ({docker_host}, max {max_sessions} sessions)");
    Ok(())
}

pub async fn remove(name: &str) -> Result<()> {
    let db = Database::open()?;

    if db.remove_host(name)? {
        println!("Host '{name}' removed.");
    } else {
        println!("Host '{name}' not found.");
    }
    Ok(())
}

pub async fn list() -> Result<()> {
    let db = Database::open()?;
    let hosts = db.list_hosts()?;

    if hosts.is_empty() {
        println!("No remote hosts registered. Add one with: mycel host add <name> <docker_host>");
        return Ok(());
    }

    println!("Registered hosts:\n");
    for host in &hosts {
        let status = if host.enabled { "enabled" } else { "disabled" };
        let load = db.count_sessions_on_host(&host.docker_host).unwrap_or(0);
        println!(
            "  {} [{}] {} ({}/{} sessions)",
            host.name, status, host.docker_host, load, host.max_sessions
        );
    }

    Ok(())
}

pub async fn enable(name: &str) -> Result<()> {
    let db = Database::open()?;
    if db.set_host_enabled(name, true)? {
        println!("Host '{name}' enabled.");
    } else {
        println!("Host '{name}' not found.");
    }
    Ok(())
}

pub async fn disable(name: &str) -> Result<()> {
    let db = Database::open()?;
    if db.set_host_enabled(name, false)? {
        println!("Host '{name}' disabled.");
    } else {
        println!("Host '{name}' not found.");
    }
    Ok(())
}
