use anyhow::{anyhow, bail, Context, Result};
use std::process::Command;

use crate::github::{GitTrackingStatus, LocalStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalRepoState {
    pub local_status: LocalStatus,
    pub tracking_status: GitTrackingStatus,
    pub has_local_git: bool,
    pub files: Vec<String>,
}

/// The repository name that contains reusable workflow definitions.
pub(crate) const WORKFLOW_SOURCE_REPO: &str = "github-actions";

#[cfg(test)]
pub(crate) fn check_local_status_no_fetch(
    base_dir: &str,
    repo_name: &str,
) -> (LocalStatus, bool, Vec<String>) {
    let state = check_local_repo_state_no_fetch(base_dir, repo_name);
    (state.local_status, state.has_local_git, state.files)
}

pub(crate) fn check_local_repo_state_no_fetch(base_dir: &str, repo_name: &str) -> LocalRepoState {
    let path = build_repo_path(base_dir, repo_name);

    if !std::path::Path::new(&path).exists() {
        return LocalRepoState {
            local_status: LocalStatus::NotFound,
            tracking_status: GitTrackingStatus::Unknown,
            has_local_git: false,
            files: vec![],
        };
    }
    let git_dir = format!("{}/.git", path);
    if !std::path::Path::new(&git_dir).exists() {
        return LocalRepoState {
            local_status: LocalStatus::NoGit,
            tracking_status: GitTrackingStatus::Unknown,
            has_local_git: false,
            files: vec![],
        };
    }

    let local_changes = get_local_changes(&path);
    let tracking_status = get_tracking_status(&path);
    let local_status = if local_changes.has_conflict {
        LocalStatus::Conflict
    } else if local_changes.has_staged {
        LocalStatus::Staging
    } else if local_changes.has_modified {
        LocalStatus::Modified
    } else {
        match tracking_status {
            GitTrackingStatus::Synced => LocalStatus::Clean,
            GitTrackingStatus::Behind { .. } => LocalStatus::Pullable,
            _ => LocalStatus::Other,
        }
    };

    LocalRepoState {
        local_status,
        tracking_status,
        has_local_git: true,
        files: local_changes.files,
    }
}

fn get_tracking_status(repo_path: &str) -> GitTrackingStatus {
    let head = Command::new("git")
        .args(["-C", repo_path, "rev-parse", "--verify", "HEAD"])
        .output();
    if !matches!(head, Ok(ref output) if output.status.success()) {
        return GitTrackingStatus::Unknown;
    }

    let upstream = Command::new("git")
        .args(["-C", repo_path, "rev-parse", "--verify", "@{u}"])
        .output();
    if !matches!(upstream, Ok(ref output) if output.status.success()) {
        return GitTrackingStatus::NoUpstream;
    }

    let counts = Command::new("git")
        .args([
            "-C",
            repo_path,
            "rev-list",
            "--left-right",
            "--count",
            "HEAD...@{u}",
        ])
        .output();
    let Ok(counts) = counts else {
        return GitTrackingStatus::Unknown;
    };
    if !counts.status.success() {
        return GitTrackingStatus::Unknown;
    }
    let counts_text = String::from_utf8_lossy(&counts.stdout);
    let mut fields = counts_text.split_whitespace();
    let (Some(ahead), Some(behind), None) = (fields.next(), fields.next(), fields.next()) else {
        return GitTrackingStatus::Unknown;
    };
    let (Ok(ahead), Ok(behind)) = (ahead.parse::<u64>(), behind.parse::<u64>()) else {
        return GitTrackingStatus::Unknown;
    };

    match (ahead, behind) {
        (0, 0) => GitTrackingStatus::Synced,
        (ahead, 0) => GitTrackingStatus::Ahead { commits: ahead },
        (0, behind) => GitTrackingStatus::Behind { commits: behind },
        (ahead, behind) => GitTrackingStatus::Diverged { ahead, behind },
    }
}

pub(crate) fn local_head_hash_no_fetch(base_dir: &str, repo_name: &str) -> String {
    let path = build_repo_path(base_dir, repo_name);
    let git_dir = format!("{}/.git", path);
    if !std::path::Path::new(&git_dir).exists() {
        return String::new();
    }
    match Command::new("git")
        .args(["-C", &path, "rev-parse", "HEAD"])
        .output()
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            eprintln!(
                "local_head_hash_no_fetch failed: repo={repo_name} path={path} stderr={stderr}"
            );
            String::new()
        }
        Err(err) => {
            eprintln!("local_head_hash_no_fetch failed: repo={repo_name} path={path} error={err}");
            String::new()
        }
    }
}

#[cfg(test)]
pub(crate) fn local_head_matches_upstream(base_dir: &str, repo_name: &str) -> bool {
    local_head_matches_upstream_with_logger(base_dir, repo_name, |msg| {
        super::cargo::append_log_message(msg)
    })
}

#[cfg(test)]
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

#[cfg(test)]
pub(super) fn local_head_matches_upstream_with_logger(
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

#[cfg(test)]
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
    let local = match Command::new("git")
        .args(["-C", repo_path, "rev-parse", "HEAD"])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if !repo_name.is_empty() {
                log_local_repo_check(
                    log_fn,
                    repo_name,
                    repo_path,
                    &format!(
                        "ローカルのコミットハッシュ取得に失敗しました: コマンド={local_command}, エラー={err}"
                    ),
                );
            }
            return None;
        }
    };
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
    let remote = match Command::new("git")
        .args(["-C", repo_path, "rev-parse", "@{u}"])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            if !repo_name.is_empty() {
                log_local_repo_check(
                    log_fn,
                    repo_name,
                    repo_path,
                    &format!(
                        "リモートから取得したコミットハッシュの取得に失敗しました: コマンド={remote_command}, エラー={err}"
                    ),
                );
            }
            return None;
        }
    };
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
            Ok(_) => Err(anyhow!("{err:#}; stashed local changes were restored")),
            Err(pop_err) => Err(anyhow!(
                "{err:#}; additionally failed to restore stashed changes: {pop_err:#}"
            )),
        };
    }

    run_git(&path, &["stash", "pop"], "git stash pop failed")
}
