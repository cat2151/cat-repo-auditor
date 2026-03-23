use std::io::Write;
use std::process::Command;

/// Returns the effective CARGO_HOME path.
fn get_cargo_home() -> String {
    std::env::var("CARGO_HOME").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        format!("{home}/.cargo")
    })
}

/// Append a timestamped error message to the unified local log file.
fn append_error_log(msg: &str) {
    let log_path = crate::config::Config::log_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(f, "[{now}] {msg}");
    }
}

/// Get the installed binary names for a git-installed crate from .crates2.json.
/// Returns None if not found.
pub(crate) fn get_cargo_bins(owner: &str, repo_name: &str) -> Option<Vec<String>> {
    let cargo_home = get_cargo_home();
    let crates2_path = format!("{cargo_home}/.crates2.json");

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    let needle = format!("git+https://github.com/{owner}/{repo_name}#");

    for (key, val) in installs {
        let src = key.trim_end_matches(')');
        if src.contains(needle.as_str()) {
            let bins = val.get("bins")?.as_array()?;
            return Some(
                bins.iter()
                    .filter_map(|b| b.as_str().map(|s| s.to_string()))
                    .collect(),
            );
        }
    }
    None
}

/// Compare the commit hash of a `cargo install --git` entry against local HEAD.
///
/// Method:
///   1. Parse `.crates2.json` for the matching entry to get the crate (app) name.
///   2. Find `$CARGO_HOME/git/checkouts/<app_name>-*` (prefix match with "-" delimiter).
///      Multiple matches → call `log_fn` and return None.
///   3. Sort sub-directories of the checkout by modification timestamp; run `git rev-parse HEAD`
///      in the most recently modified one to obtain the installed commit hash.
///   4. Run `git rev-parse HEAD` in the local clone and compare.
///
/// Returns:
///   None                         – repo not installed via `cargo install --git`, OR
///                                  .crates2.json is missing/unreadable/unparseable, OR
///                                  checkout directory not found, OR
///                                  `git rev-parse HEAD` failed
///   Some((true,  inst, local))   – installed hash == local HEAD
///   Some((false, inst, local))   – installed hash != local HEAD (stale install)
pub(crate) fn check_cargo_git_install(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
) -> Option<(bool, String, String)> {
    check_cargo_git_install_inner(owner, repo_name, base_dir, &get_cargo_home(), |msg| {
        append_error_log(msg)
    })
}

/// Internal function exposed for testing.
pub(crate) fn check_cargo_git_install_inner(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    mut log_fn: impl FnMut(&str),
) -> Option<(bool, String, String)> {
    let crates2_path = std::path::Path::new(cargo_home).join(".crates2.json");
    log_fn(&format!(
        "cargo check: cargo install metadata file: {}",
        crates2_path.display()
    ));

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    let needle = format!("git+https://github.com/{owner}/{repo_name}#");

    let app_name = installs.keys().find_map(|key| {
        let src = key.trim_end_matches(')');
        if !src.contains(needle.as_str()) {
            return None;
        }
        key.split_whitespace().next().map(|s| s.to_string())
    })?;

    let checkouts_dir = std::path::Path::new(cargo_home)
        .join("git")
        .join("checkouts");
    let prefix_with_dash = format!("{app_name}-");
    let matches: Vec<std::path::PathBuf> = match std::fs::read_dir(&checkouts_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                let name = e.file_name();
                let s = name.to_string_lossy();
                s.as_ref() == app_name.as_str() || s.starts_with(prefix_with_dash.as_str())
            })
            .map(|e| e.path())
            .collect(),
        Err(_) => return None,
    };

    if matches.len() > 1 {
        log_fn(&format!(
            "cargo check: multiple checkouts found for '{}': {:?}",
            app_name,
            matches
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
        ));
        return None;
    }

    let checkout_base = matches.into_iter().next()?;

    let sub_dir = std::fs::read_dir(&checkout_base)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let mtime = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            (mtime, e.path())
        })
        .max_by(|(mt_a, pa), (mt_b, pb)| mt_a.cmp(mt_b).then_with(|| pa.cmp(pb)))?
        .1;
    log_fn(&format!(
        "cargo check: installed hash checkout dir: {}",
        sub_dir.display()
    ));

    log_fn(&format!(
        "cargo check: installed hash source command: git -C {} rev-parse HEAD",
        sub_dir.display()
    ));
    let out = Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let installed_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if installed_hash.is_empty() {
        return None;
    }

    let repo_path = format!(
        "{}/{}",
        base_dir.trim_end_matches(|c| c == '/' || c == '\\'),
        repo_name
    );
    let out = Command::new("git")
        .args(["-C", &repo_path, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let local_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

    Some((installed_hash == local_hash, installed_hash, local_hash))
}

#[cfg(test)]
#[path = "github_local_cargo_tests.rs"]
mod tests;
