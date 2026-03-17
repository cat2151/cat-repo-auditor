use crate::github::LocalStatus;
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::Command;

// ──────────────────────────────────────────────
// Existence checks via gh REST API
// ──────────────────────────────────────────────

/// Check if README.ja.md exists in the default branch root
pub(crate) fn check_file_exists(owner: &str, repo: &str, path: &str) -> bool {
    let endpoint = format!("/repos/{owner}/{repo}/contents/{path}");
    let out = Command::new("gh")
        .args(["api", &endpoint, "--silent"])
        .output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Check if GitHub Pages is enabled for the repo
pub(crate) fn check_pages_exists(owner: &str, repo: &str) -> bool {
    let endpoint = format!("/repos/{owner}/{repo}/pages");
    let out = Command::new("gh")
        .args(["api", &endpoint, "--silent"])
        .output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Check if DeepWiki page exists (HTTP GET, 404 = false)
/// Scan local README.ja.md for a deepwiki.com link.
/// Returns true if "deepwiki.com" appears anywhere in the file.
pub(crate) fn check_deepwiki_exists(base_dir: &str, repo_name: &str) -> bool {
    // Try README.ja.md first, then README.md as fallback
    for filename in &["README.ja.md", "README.md"] {
        let path = format!("{}/{}/{}",
            base_dir.trim_end_matches(|c| c == '/' || c == '\\'), repo_name, filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("deepwiki.com") {
                return true;
            }
        }
    }
    false
}

/// Check if all 3 required workflow yml files are present in .github/workflows/
pub(crate) fn check_workflows(base_dir: &str, repo_name: &str) -> bool {
    let base = base_dir.trim_end_matches(|c| c == '/' || c == '\\');
    let wf_dir = format!("{}/{}/.github/workflows", base, repo_name);
    let required = [
        "call-translate-readme.yml",
        "call-issue-note.yml",
        "call-check-large-files.yml",
    ];
    required.iter().all(|f| {
        std::path::Path::new(&format!("{}/{}", wf_dir, f)).exists()
    })
}

/// Scan local README.ja.md for a self-referencing badge/link ("README.ja.md" text).
pub(crate) fn check_readme_ja_badge(base_dir: &str, repo_name: &str) -> bool {
    for filename in &["README.ja.md", "README.md"] {
        let path = format!("{}/{}/{}",
            base_dir.trim_end_matches(|c| c == '/' || c == '\\'), repo_name, filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("README.ja.md") {
                return true;
            }
        }
    }
    false
}

// ──────────────────────────────────────────────
// Local git status (no network)
// ──────────────────────────────────────────────

pub(crate) fn check_local_status_no_fetch(
    base_dir: &str,
    repo_name: &str,
) -> (LocalStatus, bool, Vec<String>) {
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(['/', '\\']), repo_name);

    if !std::path::Path::new(&repo_path).exists() {
        return (LocalStatus::NotFound, false, vec![]);
    }
    let git_dir = format!("{}/.git", repo_path);
    if !std::path::Path::new(&git_dir).exists() {
        return (LocalStatus::NoGit, false, vec![]);
    }

    let local  = Command::new("git").args(["-C", &repo_path, "rev-parse", "HEAD"]).output();
    let remote = Command::new("git").args(["-C", &repo_path, "rev-parse", "@{u}"]).output();
    let remote_ok = remote.as_ref().map(|r| r.status.success()).unwrap_or(false);
    let staging_files = get_staging_files(&repo_path);

    match (local, remote) {
        (Ok(l), Ok(r)) if l.status.success() && remote_ok => {
            let local_sha  = String::from_utf8_lossy(&l.stdout).trim().to_string();
            let remote_sha = String::from_utf8_lossy(&r.stdout).trim().to_string();

            if local_sha == remote_sha {
                if !staging_files.is_empty() {
                    return (LocalStatus::Staging, true, staging_files);
                }
                return (LocalStatus::Clean, true, vec![]);
            }

            let merge_base = Command::new("git")
                .args(["-C", &repo_path, "merge-base", "HEAD", "@{u}"])
                .output();

            if let Ok(mb) = merge_base {
                if mb.status.success() {
                    let base_sha = String::from_utf8_lossy(&mb.stdout).trim().to_string();
                    if base_sha == local_sha {
                        return (LocalStatus::Pullable, true, staging_files);
                    }
                }
            }
            if !staging_files.is_empty() {
                (LocalStatus::Staging, true, staging_files)
            } else {
                (LocalStatus::Other, true, vec![])
            }
        }
        (Ok(l), _) if l.status.success() => {
            if !staging_files.is_empty() {
                (LocalStatus::Staging, true, staging_files)
            } else {
                (LocalStatus::Other, true, vec![])
            }
        }
        _ => (LocalStatus::Other, true, vec![]),
    }
}

fn get_staging_files(repo_path: &str) -> Vec<String> {
    let out = Command::new("git")
        .args(["-C", repo_path, "status", "--porcelain"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect()
        }
        _ => vec![],
    }
}

// ──────────────────────────────────────────────
// git pull
// ──────────────────────────────────────────────

pub fn git_pull(base_dir: &str, repo_name: &str) -> Result<String> {
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(['/', '\\']), repo_name);
    let output = Command::new("git")
        .args(["-C", &repo_path, "pull", "--ff-only"])
        .output()
        .context("git pull failed")?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() { Ok(stdout.trim().to_string()) }
    else { bail!("{}", stderr.trim()) }
}

// ──────────────────────────────────────────────
// lazygit
// ──────────────────────────────────────────────

/// Launch an application with LeaveAlternateScreen/EnterAlternateScreen
/// to avoid terminal corruption (same pattern as lazygit).
pub fn launch_app(bin: &str, run_dir: &str) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    let status = Command::new(bin).current_dir(run_dir).status();
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    );
    match status {
        Ok(_) => Ok(()),
        Err(e) => bail!("launch failed: {e}"),
    }
}

pub fn launch_lazygit(base_dir: &str, repo_name: &str) -> Result<()> {
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(['/', '\\']), repo_name);
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    let status = Command::new("lazygit").args(["-p", &repo_path]).status();
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    );
    match status {
        Ok(_) => Ok(()),
        Err(e) => bail!("lazygit failed: {e}"),
    }
}

// ──────────────────────────────────────────────
// Open URL in browser
// ──────────────────────────────────────────────

pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    { Command::new("cmd").args(["/C", "start", "", url]).spawn().context("Failed to open browser")?; }
    #[cfg(not(target_os = "windows"))]
    { Command::new("xdg-open").arg(url).spawn().context("Failed to open browser")?; }
    Ok(())
}

// ──────────────────────────────────────────────
// Cargo install checks
// ──────────────────────────────────────────────

/// Returns the effective CARGO_HOME path.
fn get_cargo_home() -> String {
    std::env::var("CARGO_HOME").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        format!("{home}/.cargo")
    })
}

/// Append a timestamped error message to the local config logs/log.txt.
fn append_error_log(msg: &str) {
    let log_path = crate::config::Config::config_path()
        .parent()
        .map(|p| p.join("logs").join("log.txt"))
        .unwrap_or_else(|| std::path::PathBuf::from("logs/log.txt"));
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
pub fn get_cargo_bins(owner: &str, repo_name: &str) -> Option<Vec<String>> {
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
                    .collect()
            );
        }
    }
    None
}

/// Compare the commit hash of a `cargo install --git` entry against local HEAD.
///
/// Method:
///   1. Parse `.crates2.json` for the matching entry to get the crate (app) name.
///   2. Find `$CARGO_HOME/git/checkouts/<app_name>*` (prefix match).
///      Multiple matches → log error to `logs/log.txt` and return None.
///   3. Run `git rev-parse HEAD` in the first sub-directory of the checkout to
///      obtain the actually-installed commit hash.
///   4. Run `git rev-parse HEAD` in the local clone and compare.
///
/// Returns:
///   None                         – repo not installed via `cargo install --git`, OR
///                                  .crates2.json is missing/unreadable/unparseable, OR
///                                  checkout directory not found, OR
///                                  `git rev-parse HEAD` failed
///   Some((true,  inst, local))   – installed hash == local HEAD
///   Some((false, inst, local))   – installed hash != local HEAD (stale install)
pub(crate) fn check_cargo_git_install(owner: &str, repo_name: &str, base_dir: &str) -> Option<(bool, String, String)> {
    check_cargo_git_install_inner(owner, repo_name, base_dir, &get_cargo_home())
}

pub(crate) fn check_cargo_git_install_inner(owner: &str, repo_name: &str, base_dir: &str, cargo_home: &str) -> Option<(bool, String, String)> {
    let crates2_path = format!("{cargo_home}/.crates2.json");

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    let needle = format!("git+https://github.com/{owner}/{repo_name}#");

    // Get crate (app) name – first whitespace-separated token of the matching key.
    // Key format: "crate_name version (git+https://github.com/owner/repo#HASH)"
    let app_name = installs.keys().find_map(|key| {
        let src = key.trim_end_matches(')');
        if !src.contains(needle.as_str()) { return None; }
        key.split_whitespace().next().map(|s| s.to_string())
    })?;

    // Find matching checkout directory: $CARGO_HOME/git/checkouts/<app_name>*
    let checkouts_dir = std::path::Path::new(cargo_home).join("git").join("checkouts");
    let matches: Vec<std::path::PathBuf> = match std::fs::read_dir(&checkouts_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| e.file_name().to_string_lossy().starts_with(app_name.as_str()))
            .map(|e| e.path())
            .collect(),
        Err(_) => return None,
    };

    if matches.len() > 1 {
        append_error_log(&format!(
            "cargo check: multiple checkouts found for '{}': {:?}",
            app_name,
            matches.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
        ));
        return None;
    }

    let checkout_base = matches.into_iter().next()?;

    // The actual git checkout lives in the first sub-directory of checkout_base.
    let sub_dir = std::fs::read_dir(&checkout_base)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.path().is_dir())
        .map(|e| e.path())?;

    // Obtain the installed commit hash from the cargo checkout.
    let out = Command::new("git")
        .args(["-C", sub_dir.to_str()?, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let installed_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if installed_hash.is_empty() { return None; }

    // Obtain local HEAD hash.
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(|c| c == '/' || c == '\\'), repo_name);
    let out = Command::new("git")
        .args(["-C", &repo_path, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let local_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

    Some((installed_hash == local_hash, installed_hash, local_hash))
}

// ──────────────────────────────────────────────
// Self-update check
// ──────────────────────────────────────────────

/// Check if a newer version of gh-tui is available by comparing
/// crates2.json installed hash vs remote HEAD of the git repo.
/// Returns Some("owner/repo") if update is available, None if up-to-date or not installed.
pub fn check_self_update() -> Option<String> {
    let cargo_home = get_cargo_home();
    let crates2_path = format!("{cargo_home}/.crates2.json");

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    // Find gh-tui entry with git+ URL
    let (installed_hash, owner_repo) = installs.keys().find_map(|key| {
        let mut parts = key.splitn(3, ' ');
        let name = parts.next()?;
        if name != "cat-repo-auditor" { return None; }
        let src = key.trim_end_matches(')');
        let git_prefix = "git+https://github.com/";
        let idx = src.find(git_prefix)?;
        let rest = &src[idx + git_prefix.len()..];
        // rest = "owner/repo#HASH"
        let hash_idx = rest.find('#')?;
        let owner_repo = &rest[..hash_idx];
        let hash = &rest[hash_idx + 1..];
        if hash.is_empty() { return None; }
        Some((hash.to_string(), owner_repo.to_string()))
    })?;

    // Get remote HEAD via gh api
    let endpoint = format!("/repos/{owner_repo}/commits/HEAD");
    let out = Command::new("gh")
        .args(["api", &endpoint, "--jq", ".sha"])
        .output().ok()?;
    if !out.status.success() { return None; }
    let remote_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

    if remote_hash.is_empty() || remote_hash == installed_hash { return None; }
    Some(owner_repo)
}

#[cfg(test)]
#[path = "github_local_tests.rs"]
mod tests;
