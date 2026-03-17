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
#[path = "history_tests.rs"]
mod tests;
