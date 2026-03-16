use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::PathBuf};

const TEMPLATE: &str = r#"# gh-tui configuration
# GitHub owner (user or org) to list repositories for
owner = "your-github-username"

# Local base directory where repositories are cloned
local_base_dir = "C:\\Users\\you\\repos"

# Working directory when launching apps via x key.
# Defaults to Windows user Desktop if not specified.
# app_run_dir = "C:\\Users\\you\\Desktop"

# Automatically pull pullable repos on refresh (default: false)
auto_pull = false
"#;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub owner: String,
    pub local_base_dir: String,
    #[serde(default)]
    pub app_run_dir: Option<String>,
    #[serde(default)]
    pub auto_pull: bool,
}

impl Config {
    /// Returns the platform config file path:
    /// Windows: %LOCALAPPDATA%\gh-tui\config.toml
    /// Other:   ~/.config/gh-tui/config.toml
    pub fn config_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            let base = std::env::var("LOCALAPPDATA")
                .unwrap_or_else(|_| {
                    std::env::var("USERPROFILE")
                        .map(|p| format!("{p}\\AppData\\Local"))
                        .unwrap_or_else(|_| String::from("."))
                });
            PathBuf::from(base).join("cat-repo-auditor").join("config.toml")
        }
        #[cfg(not(target_os = "windows"))]
        {
            let base = std::env::var("XDG_CONFIG_HOME")
                .unwrap_or_else(|_| {
                    std::env::var("HOME")
                        .map(|h| format!("{h}/.config"))
                        .unwrap_or_else(|_| String::from("."))
                });
            PathBuf::from(base).join("cat-repo-auditor").join("config.toml")
        }
    }

    /// Returns the history file path next to config.toml
    pub fn history_path() -> PathBuf {
        Self::config_path().with_file_name("history.json")
    }

    /// Load config from platform config dir.
    /// If file doesn't exist, create it from template and return an error
    /// instructing the user to edit it.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if !path.exists() {
            // Create parent dirs
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create config dir: {}", parent.display()))?;
            }
            fs::write(&path, TEMPLATE)
                .with_context(|| format!("Failed to write template config: {}", path.display()))?;
            anyhow::bail!(
                "Config created at {}\nPlease edit it and run gh-tui again.",
                path.display()
            );
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config: {}", path.display()))?;
        Ok(config)
    }

    /// Resolve effective app_run_dir.
    /// Priority: config value → %USERPROFILE%\Desktop → "."
    pub fn resolved_app_run_dir(&self) -> String {
        if let Some(ref d) = self.app_run_dir {
            if !d.is_empty() { return d.clone(); }
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let desktop = format!("{profile}\\Desktop");
            if std::path::Path::new(&desktop).exists() { return desktop; }
        }
        if let (Ok(drive), Ok(path)) = (
            std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH"),
        ) {
            let desktop = format!("{drive}{path}\\Desktop");
            if std::path::Path::new(&desktop).exists() { return desktop; }
        }
        String::from(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_path_ends_with_history_json() {
        let path = Config::history_path();
        assert_eq!(path.file_name().unwrap(), "history.json");
    }

    #[test]
    fn resolved_app_run_dir_returns_config_value_when_set() {
        let config = Config {
            owner: String::from("owner"),
            local_base_dir: String::from("/base"),
            app_run_dir: Some(String::from("/custom/dir")),
            auto_pull: false,
        };
        assert_eq!(config.resolved_app_run_dir(), "/custom/dir");
    }

    #[test]
    fn resolved_app_run_dir_falls_back_when_not_set() {
        let config = Config {
            owner: String::from("owner"),
            local_base_dir: String::from("/base"),
            app_run_dir: None,
            auto_pull: false,
        };
        let result = config.resolved_app_run_dir();
        assert!(!result.is_empty());
    }
}
