use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::bank;
use crate::db::Database;
use crate::worktree;

pub async fn run(name: &str, output: Option<PathBuf>) -> Result<()> {
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

    let metadata = match bank::read_metadata(&project.name, name)? {
        Some(metadata) => metadata,
        None => bank::BankMetadata::new(project.name.clone(), name.to_string(), None, None),
    };

    let output_path = output.unwrap_or_else(|| {
        current_dir.join(format!("{}-{}.mycel-bank.tar.gz", project.name, name))
    });
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).context("Failed to create export directory")?;
    }

    bank::export_bundle(&bundle_path, &metadata, &output_path)?;
    println!("Exported bank to {}", output_path.display());

    Ok(())
}
