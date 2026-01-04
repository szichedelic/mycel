use anyhow::Result;

use crate::db::Database;

pub async fn run() -> Result<()> {
    let db = Database::open()?;
    let projects = db.list_projects()?;

    if projects.is_empty() {
        println!("No projects registered. Run 'mycel init' in a git repository.");
        return Ok(());
    }

    println!("Registered projects:\n");
    for project in projects {
        let session_count = db.count_sessions_for_project(project.id)?;
        println!(
            "  {} ({}) - {} session{}",
            project.name,
            project.path.display(),
            session_count,
            if session_count == 1 { "" } else { "s" }
        );
    }

    Ok(())
}
