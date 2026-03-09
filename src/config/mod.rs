use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_terminal")]
    pub terminal: String,
    #[serde(default)]
    pub editor: Option<String>,
    #[serde(default = "default_refresh_rate")]
    pub refresh_rate: u64,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub backends: BTreeMap<String, BackendConfig>,
    #[serde(default)]
    pub hosts: Vec<HostConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    pub name: String,
    pub docker_host: String,
    #[serde(default = "default_max_sessions")]
    pub max_sessions: i64,
}

fn default_max_sessions() -> i64 {
    4
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
            backend: None,
            backends: BTreeMap::new(),
            hosts: Vec::new(),
        }
    }
}

impl GlobalConfig {
    pub fn load() -> Result<Self> {
        let home_dir = dirs::home_dir().context("Could not find home directory")?;
        let config_paths = [
            home_dir.join(".mycel").join("config.toml"),
            home_dir.join(".mycelrc"),
        ];

        for config_path in &config_paths {
            if !config_path.exists() {
                continue;
            }

            let content =
                fs::read_to_string(config_path).context("Failed to read global config")?;
            return toml::from_str(&content).context("Failed to parse global config");
        }

        Ok(Self::default())
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
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub backends: BTreeMap<String, BackendConfig>,
    #[serde(default)]
    pub templates: BTreeMap<String, TemplateConfig>,
    #[serde(default)]
    pub symlink_paths: Vec<String>,
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
            backend: None,
            backends: BTreeMap::new(),
            templates: BTreeMap::new(),
            symlink_paths: Vec::new(),
        }
    }
}

impl ProjectConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        if let Some(external_path) = external_config_path_from_git_root(project_root) {
            if external_path.exists() {
                let content = fs::read_to_string(&external_path)
                    .context("Failed to read external project config")?;
                return toml::from_str(&content).context("Failed to parse external project config");
            }
        }

        let config_path = project_root.join(".mycel.toml");
        if config_path.exists() {
            let content =
                fs::read_to_string(&config_path).context("Failed to read project config")?;
            return toml::from_str(&content).context("Failed to parse project config");
        }

        Ok(Self::default())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }
        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(path, content).context("Failed to write config file")?;
        Ok(())
    }

    pub fn exists(project_root: &Path) -> bool {
        if let Some(external_path) = external_config_path_from_git_root(project_root) {
            if external_path.exists() {
                return true;
            }
        }
        project_root.join(".mycel.toml").exists()
    }
}

pub fn get_external_config_path(project_name: &str) -> Option<PathBuf> {
    external_config_path(project_name)
}

fn external_config_path(project_name: &str) -> Option<PathBuf> {
    let config_dir = dirs::config_dir()?.join("mycel").join("projects");
    Some(config_dir.join(format!("{project_name}.toml")))
}

fn external_config_path_from_git_root(git_root: &Path) -> Option<PathBuf> {
    let project_name = git_root.file_name()?.to_str()?;
    external_config_path(project_name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    #[serde(default)]
    pub branch_prefix: Option<String>,
    #[serde(default)]
    pub setup: Vec<String>,
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct ResolvedBackend {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

pub fn resolve_backend(
    global: &GlobalConfig,
    project: &ProjectConfig,
    override_backend: Option<&str>,
) -> Result<ResolvedBackend> {
    let selected = override_backend
        .filter(|name| !name.trim().is_empty())
        .or(project.backend.as_deref())
        .or(global.backend.as_deref())
        .unwrap_or("claude");

    let mut resolved = if let Some(default) = default_backend(selected) {
        default
    } else if project.backends.contains_key(selected) || global.backends.contains_key(selected) {
        ResolvedBackend {
            name: selected.to_string(),
            command: selected.to_string(),
            args: Vec::new(),
        }
    } else {
        let available = available_backend_names(global, project);
        let list = if available.is_empty() {
            "none configured".to_string()
        } else {
            available.join(", ")
        };
        bail!("Unknown backend '{selected}'. Available backends: {list}");
    };

    if let Some(config) = global.backends.get(selected) {
        apply_backend_override(&mut resolved, config);
    }
    if let Some(config) = project.backends.get(selected) {
        apply_backend_override(&mut resolved, config);
    }

    Ok(resolved)
}

pub fn available_backend_names(global: &GlobalConfig, project: &ProjectConfig) -> Vec<String> {
    let mut names: BTreeSet<String> = default_backend_configs().keys().cloned().collect();
    names.extend(global.backends.keys().cloned());
    names.extend(project.backends.keys().cloned());
    names.into_iter().collect()
}

fn apply_backend_override(backend: &mut ResolvedBackend, config: &BackendConfig) {
    if let Some(command) = config
        .command
        .as_deref()
        .filter(|cmd| !cmd.trim().is_empty())
    {
        backend.command = command.to_string();
    }
    if let Some(args) = config.args.as_ref() {
        backend.args = args.clone();
    }
}

fn default_backend(name: &str) -> Option<ResolvedBackend> {
    default_backend_configs()
        .get(name)
        .map(|config| ResolvedBackend {
            name: name.to_string(),
            command: config.command.clone().unwrap_or_else(|| name.to_string()),
            args: config.args.clone().unwrap_or_default(),
        })
}

fn default_backend_configs() -> BTreeMap<String, BackendConfig> {
    let mut configs = BTreeMap::new();
    configs.insert(
        "claude".to_string(),
        BackendConfig {
            command: Some("claude".to_string()),
            args: Some(Vec::new()),
        },
    );
    configs.insert(
        "codex".to_string(),
        BackendConfig {
            command: Some("codex".to_string()),
            args: Some(Vec::new()),
        },
    );
    configs
}
