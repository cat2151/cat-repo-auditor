use crate::github::LocalStatus;
use anyhow::{bail, Context, Result};
use std::process::Command;

#[path = "github_local_cargo.rs"]
mod cargo;

pub(crate) use cargo::{append_cargo_check_results, check_cargo_git_install, get_cargo_bins};

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
        let path = format!(
            "{}/{}/{}",
            base_dir.trim_end_matches(|c| c == '/' || c == '\\'),
            repo_name,
            filename
        );
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
    required
        .iter()
        .all(|f| std::path::Path::new(&format!("{}/{}", wf_dir, f)).exists())
}

/// Scan local README.ja.md for a self-referencing badge/link ("README.ja.md" text).
pub(crate) fn check_readme_ja_badge(base_dir: &str, repo_name: &str) -> bool {
    for filename in &["README.ja.md", "README.md"] {
        let path = format!(
            "{}/{}/{}",
            base_dir.trim_end_matches(|c| c == '/' || c == '\\'),
            repo_name,
            filename
        );
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
    let path = build_repo_path(base_dir, repo_name);

    if !std::path::Path::new(&path).exists() {
        return (LocalStatus::NotFound, false, vec![]);
    }
    let git_dir = format!("{}/.git", path);
    if !std::path::Path::new(&git_dir).exists() {
        return (LocalStatus::NoGit, false, vec![]);
    }

    let local_changes = get_local_changes(&path);
    if local_changes.has_conflict {
        return (LocalStatus::Conflict, true, local_changes.files);
    }
    if local_changes.has_staged {
        return (LocalStatus::Staging, true, local_changes.files);
    }
    if local_changes.has_modified {
        return (LocalStatus::Modified, true, local_changes.files);
    }

    match local_and_upstream_heads(&path) {
        Some((local_sha, remote_sha)) => {
            if local_sha == remote_sha {
                return (LocalStatus::Clean, true, vec![]);
            }

            let merge_base = Command::new("git")
                .args(["-C", &path, "merge-base", "HEAD", "@{u}"])
                .output();

            if let Ok(mb) = merge_base {
                if mb.status.success() {
                    let base_sha = String::from_utf8_lossy(&mb.stdout).trim().to_string();
                    if base_sha == local_sha {
                        return (LocalStatus::Pullable, true, vec![]);
                    }
                }
            }
            (LocalStatus::Other, true, vec![])
        }
        None => (LocalStatus::Other, true, vec![]),
    }
}

pub(crate) fn local_head_matches_upstream(base_dir: &str, repo_name: &str) -> bool {
    local_head_matches_upstream_with_logger(base_dir, repo_name, |msg| {
        cargo::append_log_message(msg)
    })
}

fn log_local_repo_check(
    log_fn: &mut impl FnMut(&str),
    repo_name: &str,
    repo_path: &str,
    result: &str,
) {
    log_fn(&format!(
        "local repo check: リポジトリ={repo_name} パス={repo_path} 結果={result}"
    ));
}

fn local_head_matches_upstream_with_logger(
    base_dir: &str,
    repo_name: &str,
    mut log_fn: impl FnMut(&str),
) -> bool {
    let path = build_repo_path(base_dir, repo_name);
    log_local_repo_check(
        &mut log_fn,
        repo_name,
        &path,
        "開始: ローカルとリモートのコミットハッシュ比較を開始します",
    );
    let result = match local_and_upstream_heads_with_logger(repo_name, &path, &mut log_fn) {
        Some((local_sha, remote_sha)) => {
            let matches = local_sha == remote_sha;
            log_local_repo_check(
                &mut log_fn,
                repo_name,
                &path,
                &format!(
                    "ローカルとリモートのコミットハッシュ比較結果={}",
                    if matches { "一致" } else { "不一致" }
                ),
            );
            matches
        }
        None => {
            log_local_repo_check(
                &mut log_fn,
                repo_name,
                &path,
                "ローカルまたはリモートのコミットハッシュを取得できなかったため、比較結果を判定できません",
            );
            false
        }
    };
    log_local_repo_check(
        &mut log_fn,
        repo_name,
        &path,
        &format!(
            "終了: ローカル repo check を完了しました (比較結果={})",
            if result {
                "一致"
            } else {
                "不一致または判定不能"
            }
        ),
    );
    result
}

struct LocalChanges {
    files: Vec<String>,
    has_conflict: bool,
    has_staged: bool,
    has_modified: bool,
}

fn get_local_changes(repo_path: &str) -> LocalChanges {
    let out = Command::new("git")
        .args(["-C", repo_path, "status", "--porcelain"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let mut files = Vec::new();
            let mut has_conflict = false;
            let mut has_staged = false;
            let mut has_modified = false;

            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if line.trim().is_empty() {
                    continue;
                }
                files.push(line.to_string());

                let status = line.as_bytes();
                if status.len() < 2 {
                    has_modified = true;
                    continue;
                }

                let x = status[0] as char;
                let y = status[1] as char;

                if is_unmerged_status(x, y) {
                    has_conflict = true;
                    continue;
                }

                if x == '?' && y == '?' {
                    has_modified = true;
                    continue;
                }
                if x != ' ' {
                    has_staged = true;
                }
                if y != ' ' {
                    has_modified = true;
                }
            }

            LocalChanges {
                files,
                has_conflict,
                has_staged,
                has_modified,
            }
        }
        _ => LocalChanges {
            files: vec![],
            has_conflict: false,
            has_staged: false,
            has_modified: false,
        },
    }
}

fn local_and_upstream_heads(repo_path: &str) -> Option<(String, String)> {
    fn noop(_: &str) {}
    let mut noop = noop;
    local_and_upstream_heads_with_logger("", repo_path, &mut noop)
}

fn local_and_upstream_heads_with_logger(
    repo_name: &str,
    repo_path: &str,
    log_fn: &mut impl FnMut(&str),
) -> Option<(String, String)> {
    let local_command = format!("git -C {repo_path} rev-parse HEAD");
    if !repo_name.is_empty() {
        log_local_repo_check(
            log_fn,
            repo_name,
            repo_path,
            &format!("ローカルのコミットハッシュ取得を開始します: コマンド={local_command}"),
        );
    }
    let local = Command::new("git")
        .args(["-C", repo_path, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !local.status.success() {
        if !repo_name.is_empty() {
            log_local_repo_check(
                log_fn,
                repo_name,
                repo_path,
                &format!("ローカルのコミットハッシュ取得に失敗しました: コマンド={local_command}"),
            );
        }
        return None;
    }
    let local_sha = String::from_utf8_lossy(&local.stdout).trim().to_string();
    if !repo_name.is_empty() {
        log_local_repo_check(
            log_fn,
            repo_name,
            repo_path,
            &format!("ローカルのコミットハッシュを取得しました: {local_sha}"),
        );
    }

    let remote_command = format!("git -C {repo_path} rev-parse @{{u}}");
    if !repo_name.is_empty() {
        log_local_repo_check(
            log_fn,
            repo_name,
            repo_path,
            &format!(
                "リモートから取得したコミットハッシュの取得を開始します: コマンド={remote_command}"
            ),
        );
    }
    let remote = Command::new("git")
        .args(["-C", repo_path, "rev-parse", "@{u}"])
        .output()
        .ok()?;
    if !remote.status.success() {
        if !repo_name.is_empty() {
            log_local_repo_check(
                log_fn,
                repo_name,
                repo_path,
                &format!(
                    "リモートから取得したコミットハッシュの取得に失敗しました: コマンド={remote_command}"
                ),
            );
        }
        return None;
    }
    let remote_sha = String::from_utf8_lossy(&remote.stdout).trim().to_string();
    if !repo_name.is_empty() {
        log_local_repo_check(
            log_fn,
            repo_name,
            repo_path,
            &format!("リモートから取得したコミットハッシュを取得しました: {remote_sha}"),
        );
    }

    Some((local_sha, remote_sha))
}

fn is_unmerged_status(x: char, y: char) -> bool {
    matches!(
        (x, y),
        ('A', 'A') | ('D', 'D') | ('U', 'D') | ('D', 'U') | ('A', 'U') | ('U', 'A') | ('U', 'U')
    )
}

fn build_repo_path(base_dir: &str, repo_name: &str) -> String {
    format!("{}/{}", base_dir.trim_end_matches(['/', '\\']), repo_name)
}

fn run_git(repo_path: &str, args: &[&str], context_msg: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .with_context(|| context_msg.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        bail!("{}", if stderr.is_empty() { stdout } else { stderr })
    }
}

// ──────────────────────────────────────────────
// git pull
// ──────────────────────────────────────────────

pub fn git_pull(base_dir: &str, repo_name: &str) -> Result<String> {
    let path = build_repo_path(base_dir, repo_name);
    let local_changes = get_local_changes(&path);
    if local_changes.has_conflict {
        bail!("repository has unresolved conflicts");
    }

    let needs_stash = local_changes.has_staged || local_changes.has_modified;
    if !needs_stash {
        return run_git(&path, &["pull", "--ff-only"], "git pull failed");
    }

    run_git(
        &path,
        &[
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "catrepo auto-pull",
        ],
        "git stash push failed",
    )?;

    let pull_result = run_git(&path, &["pull", "--ff-only"], "git pull failed");
    if let Err(err) = pull_result {
        return match run_git(&path, &["stash", "pop"], "git stash pop failed") {
            Ok(_) => Err(anyhow::anyhow!(
                "{err:#}; stashed local changes were restored"
            )),
            Err(pop_err) => Err(anyhow::anyhow!(
                "{err:#}; additionally failed to restore stashed changes: {pop_err:#}"
            )),
        };
    }

    run_git(&path, &["stash", "pop"], "git stash pop failed")
}

// ──────────────────────────────────────────────
// lazygit
// ──────────────────────────────────────────────

/// Launch an application with LeaveAlternateScreen/EnterAlternateScreen
/// to avoid terminal corruption (same pattern as lazygit).
pub fn launch_app_with_args(bin: &str, args: &[&str], run_dir: &str) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    let status = Command::new(bin).args(args).current_dir(run_dir).status();
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
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("Failed to open browser")?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("Failed to open browser")?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "github_local_tests.rs"]
mod tests;
