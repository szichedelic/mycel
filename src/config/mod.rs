use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_terminal")]
    pub terminal: String,
    #[serde(default)]
    pub editor: Option<String>,
    #[serde(default = "default_refresh_rate")]
    pub refresh_rate: u64,
}

fn default_terminal() -> String {
    "default".to_string()
}

fn default_refresh_rate() -> u64 {
    100
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            terminal: default_terminal(),
            editor: None,
            refresh_rate: default_refresh_rate(),
        }
    }
}

impl GlobalConfig {
    pub fn load() -> Result<Self> {
        let config_path = dirs::home_dir()
            .context("Could not find home directory")?
            .join(".mycelrc");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .context("Failed to read global config")?;

        toml::from_str(&content).context("Failed to parse global config")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default = "default_base_branch")]
    pub base_branch: String,
    #[serde(default)]
    pub setup: Vec<String>,
    #[serde(default = "default_worktree_dir")]
    pub worktree_dir: String,
    #[serde(default)]
    pub max_sessions: Option<u32>,
}

fn default_base_branch() -> String {
    "main".to_string()
}

fn default_worktree_dir() -> String {
    "../.mycel-worktrees".to_string()
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            base_branch: default_base_branch(),
            setup: Vec::new(),
            worktree_dir: default_worktree_dir(),
            max_sessions: None,
        }
    }
}

impl ProjectConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".mycel.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .context("Failed to read project config")?;

        toml::from_str(&content).context("Failed to parse project config")
    }
}
