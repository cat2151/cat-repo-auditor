use crate::github::{RateLimit, RepoInfo};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs, io,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct History {
    /// ETag per query key (e.g. owner name)
    pub etags: HashMap<String, String>,
    /// Cached repos
    pub repos: Vec<RepoInfo>,
    /// Last rate limit info
    pub rate_limit: Option<RateLimit>,
}

fn history_file_lock() -> &'static Mutex<()> {
    static HISTORY_FILE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    HISTORY_FILE_LOCK.get_or_init(|| Mutex::new(()))
}

fn write_atomic(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp_path = path.with_extension(format!("tmp-{}-{unique}", std::process::id()));
    fs::write(&tmp_path, content)?;

    match fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            if let Err(remove_err) = fs::remove_file(&tmp_path) {
                eprintln!(
                    "history temp file cleanup failed: path={} error={remove_err}",
                    tmp_path.display()
                );
            }
            Err(err)
        }
    }
}

impl History {
    pub fn load(path: &str) -> Result<Self> {
        let _guard = history_file_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let content = fs::read_to_string(path)?;
        let h: History = serde_json::from_str(&content)?;
        Ok(h)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let _guard = history_file_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let content = serde_json::to_string_pretty(self)?;
        write_atomic(Path::new(path), &content)?;
        Ok(())
    }

    pub fn update(path: &str, f: impl FnOnce(&mut Self)) -> Result<()> {
        let _guard = history_file_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let history_path = Path::new(path);
        let mut history = match fs::read_to_string(history_path) {
            Ok(content) => serde_json::from_str(&content)?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => History::default(),
            Err(err) => return Err(err.into()),
        };
        f(&mut history);
        let content = serde_json::to_string_pretty(&history)?;
        write_atomic(history_path, &content)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
