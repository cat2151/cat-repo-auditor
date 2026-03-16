use crate::github::{RateLimit, RepoInfo};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct History {
    /// ETag per query key (e.g. owner name)
    pub etags: HashMap<String, String>,
    /// Cached repos
    pub repos: Vec<RepoInfo>,
    /// Last rate limit info
    pub rate_limit: Option<RateLimit>,
}

impl History {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let h: History = serde_json::from_str(&content)?;
        Ok(h)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = std::env::temp_dir()
            .join(format!("cat_repo_auditor_history_test_{}.json", std::process::id()));
        let path_str = tmp.to_str().unwrap();

        let mut history = History::default();
        history.etags.insert(String::from("owner"), String::from("etag123"));

        history.save(path_str).unwrap();
        let loaded = History::load(path_str).unwrap();

        assert_eq!(loaded.etags.get("owner").unwrap(), "etag123");
        assert!(loaded.repos.is_empty());
        assert!(loaded.rate_limit.is_none());
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = History::load("/nonexistent/path/history.json");
        assert!(result.is_err());
    }
}
