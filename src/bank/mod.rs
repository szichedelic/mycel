use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tar::{Archive, Builder, Header};

/// Get the bank directory for a project
pub fn bank_dir(project_name: &str) -> Result<PathBuf> {
    let base = dirs::home_dir()
        .context("Could not find home directory")?
        .join(".mycel")
        .join("bank")
        .join(project_name);

    fs::create_dir_all(&base)?;
    Ok(base)
}

/// Get the bundle path for a banked session
pub fn bundle_path(project_name: &str, session_name: &str) -> Result<PathBuf> {
    Ok(bank_dir(project_name)?.join(format!("{session_name}.bundle")))
}

pub fn metadata_path(project_name: &str, session_name: &str) -> Result<PathBuf> {
    Ok(bank_dir(project_name)?.join(format!("{session_name}.metadata.toml")))
}

/// Create a git bundle from a branch
pub fn create_bundle(
    git_root: &Path,
    branch_name: &str,
    base_branch: &str,
    bundle_path: &Path,
) -> Result<()> {
    // Check if there are commits to bundle
    let output = Command::new("git")
        .args([
            "rev-list",
            "--count",
            &format!("{base_branch}..{branch_name}"),
        ])
        .current_dir(git_root)
        .output()
        .context("Failed to check commit count")?;

    let count: i32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);

    if count == 0 {
        bail!("No commits to bank. Did you forget to commit your work?");
    }

    // Create the bundle
    let status = Command::new("git")
        .args([
            "bundle",
            "create",
            &bundle_path.to_string_lossy(),
            &format!("{base_branch}..{branch_name}"),
        ])
        .current_dir(git_root)
        .status()
        .context("Failed to create bundle")?;

    if !status.success() {
        bail!("git bundle create failed");
    }

    println!("Banked {count} commits");
    Ok(())
}

/// Verify a bundle is valid
pub fn verify_bundle(bundle_path: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["bundle", "verify", &bundle_path.to_string_lossy()])
        .status()
        .context("Failed to verify bundle")?;

    if !status.success() {
        bail!("Bundle verification failed");
    }

    Ok(())
}

/// Restore a branch from a bundle
pub fn restore_bundle(git_root: &Path, bundle_path: &Path, branch_name: &str) -> Result<()> {
    // Fetch the branch from the bundle
    let status = Command::new("git")
        .args([
            "fetch",
            &bundle_path.to_string_lossy(),
            &format!("{branch_name}:{branch_name}"),
        ])
        .current_dir(git_root)
        .status()
        .context("Failed to restore from bundle")?;

    if !status.success() {
        bail!("git fetch from bundle failed");
    }

    Ok(())
}

/// List all banked bundles for a project
pub fn list_banked(project_name: &str) -> Result<Vec<BankedItem>> {
    let dir = bank_dir(project_name)?;
    let mut items = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "bundle").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let metadata = fs::metadata(&path)?;
                let size = metadata.len();

                items.push(BankedItem { name, path, size });
            }
        }
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// Delete a bundle file
pub fn delete_bundle(bundle_path: &Path) -> Result<()> {
    fs::remove_file(bundle_path).context("Failed to delete bundle")?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankMetadata {
    pub version: u32,
    pub project_name: String,
    pub session_name: String,
    pub note: Option<String>,
    pub created_at_unix: Option<i64>,
    pub banked_at_unix: i64,
    #[serde(default)]
    pub exported_at_unix: Option<i64>,
}

impl BankMetadata {
    pub fn new(
        project_name: impl Into<String>,
        session_name: impl Into<String>,
        note: Option<String>,
        created_at_unix: Option<i64>,
    ) -> Self {
        Self {
            version: 1,
            project_name: project_name.into(),
            session_name: session_name.into(),
            note,
            created_at_unix,
            banked_at_unix: current_unix_timestamp(),
            exported_at_unix: None,
        }
    }
}

pub struct ImportedBundle {
    pub session_name: String,
    pub bundle_path: PathBuf,
}

pub fn write_metadata(
    project_name: &str,
    session_name: &str,
    metadata: &BankMetadata,
) -> Result<PathBuf> {
    let path = metadata_path(project_name, session_name)?;
    let content = toml::to_string(metadata).context("Failed to serialize bank metadata")?;
    fs::write(&path, content).context("Failed to write bank metadata")?;
    Ok(path)
}

pub fn read_metadata(project_name: &str, session_name: &str) -> Result<Option<BankMetadata>> {
    let path = metadata_path(project_name, session_name)?;
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).context("Failed to read bank metadata")?;
    let metadata = toml::from_str(&content).context("Failed to parse bank metadata")?;
    Ok(Some(metadata))
}

pub fn delete_metadata(project_name: &str, session_name: &str) -> Result<()> {
    let path = metadata_path(project_name, session_name)?;
    if path.exists() {
        fs::remove_file(path).context("Failed to delete bank metadata")?;
    }
    Ok(())
}

pub fn export_bundle(bundle_path: &Path, metadata: &BankMetadata, output_path: &Path) -> Result<()> {
    let file = File::create(output_path).context("Failed to create export file")?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);

    let mut export_metadata = metadata.clone();
    export_metadata.exported_at_unix = Some(current_unix_timestamp());
    let metadata_content =
        toml::to_string(&export_metadata).context("Failed to serialize export metadata")?;
    let mut header = Header::new_gnu();
    header.set_size(metadata_content.as_bytes().len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, "metadata.toml", metadata_content.as_bytes())?;

    builder.append_path_with_name(bundle_path, "bundle.bundle")?;
    builder.finish()?;

    Ok(())
}

pub fn import_bundle(
    archive_path: &Path,
    project_name: &str,
    name_override: Option<&str>,
    force: bool,
) -> Result<ImportedBundle> {
    let file = File::open(archive_path).context("Failed to open archive")?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let mut metadata: Option<BankMetadata> = None;
    let mut session_name = name_override.map(|name| name.to_string());
    let mut bundle_output_path: Option<PathBuf> = None;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry
            .path()
            .context("Failed to read archive entry path")?
            .to_string_lossy()
            .to_string();

        if entry_path.ends_with("metadata.toml") {
            let mut contents = String::new();
            entry
                .read_to_string(&mut contents)
                .context("Failed to read metadata entry")?;
            let parsed: BankMetadata =
                toml::from_str(&contents).context("Failed to parse metadata")?;
            if session_name.is_none() {
                session_name = Some(parsed.session_name.clone());
            }
            metadata = Some(parsed);
            continue;
        }

        if entry_path.ends_with("bundle.bundle") {
            let session_name = session_name
                .clone()
                .context("Archive missing metadata; provide --name")?;
            let output_path = bundle_path(project_name, &session_name)?;
            if output_path.exists() && !force {
                bail!("Bundle already exists: {}", output_path.display());
            }

            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent).context("Failed to create bank directory")?;
            }

            let mut output =
                File::create(&output_path).context("Failed to create bundle file")?;
            std::io::copy(&mut entry, &mut output)
                .context("Failed to write bundle file")?;
            bundle_output_path = Some(output_path);
        }
    }

    let session_name = session_name.context("Archive missing metadata; provide --name")?;
    let bundle_path = bundle_output_path.context("Archive missing bundle data")?;
    let mut metadata = metadata.unwrap_or_else(|| {
        BankMetadata::new(project_name.to_string(), session_name.clone(), None, None)
    });
    metadata.project_name = project_name.to_string();
    metadata.session_name = session_name.clone();
    metadata.exported_at_unix = None;
    write_metadata(project_name, &session_name, &metadata)?;

    Ok(ImportedBundle {
        session_name,
        bundle_path,
    })
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[derive(Debug)]
pub struct BankedItem {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
}

impl BankedItem {
    pub fn size_human(&self) -> String {
        if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{:.1} KB", self.size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        }
    }
}
