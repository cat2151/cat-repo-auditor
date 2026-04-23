use std::io::Write;
use std::path::Path;
use std::process::Output;

#[path = "github_local_cargo_bins.rs"]
mod bins;
#[path = "github_local_cargo_hash.rs"]
mod hash;

pub(crate) use bins::get_cargo_bins;
pub(crate) use hash::{
    check_cargo_git_install, check_cargo_git_install_status, CargoGitInstallCheck,
};

#[cfg(test)]
use bins::get_cargo_bins_inner;
#[cfg(test)]
use hash::{
    check_cargo_git_install_inner, check_cargo_git_install_inner_with_remote_hash,
    check_cargo_git_install_with_remote_hash_and_logger,
};

/// Returns the effective CARGO_HOME path.
fn get_cargo_home() -> String {
    std::env::var("CARGO_HOME").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        format!("{home}/.cargo")
    })
}

fn append_log_messages_to_path(
    log_path: &Path,
    messages: impl IntoIterator<Item = impl AsRef<str>>,
) {
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
    {
        // 同一 batch 内のログは同じタイムスタンプで記録し、一連の処理として識別しやすくする。
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        for msg in messages {
            let _ = writeln!(f, "[{now}] {}", msg.as_ref());
        }
    }
}

/// Append one or more timestamped log messages to the unified local log file.
pub(super) fn append_log_messages(messages: impl IntoIterator<Item = impl AsRef<str>>) {
    append_log_messages_to_path(&crate::config::Config::log_path(), messages);
}

pub(super) fn append_log_message(msg: &str) {
    append_log_messages(std::iter::once(msg));
}

pub(crate) fn append_cargo_check_results(owner: &str, results: &[(String, String)]) {
    append_log_messages(results.iter().map(|(repo_name, result)| {
        format!("cargo check: リポジトリ={owner}/{repo_name} 結果={result}")
    }));
}

pub(crate) fn append_cargo_check_after_auto_update_log(
    repo_full_name: &str,
    messages: impl IntoIterator<Item = impl AsRef<str>>,
) {
    append_cargo_check_after_auto_update_log_for_path(
        &crate::config::Config::cargo_check_after_auto_update_log_path(),
        repo_full_name,
        messages,
    );
}

pub(super) fn append_cargo_check_after_auto_update_log_for_path(
    path: &Path,
    repo_full_name: &str,
    messages: impl IntoIterator<Item = impl AsRef<str>>,
) {
    let mut lines = vec![format!("========== {repo_full_name} ==========")];
    lines.extend(
        messages
            .into_iter()
            .map(|message| message.as_ref().to_string()),
    );
    append_log_messages_to_path(path, lines);
}

fn log_cargo_check_path_result(
    log_fn: &mut impl FnMut(&str),
    owner: &str,
    repo_name: &str,
    path: &Path,
    result: &str,
) {
    log_fn(&format!(
        "cargo check: リポジトリ={owner}/{repo_name} パス={} 結果={result}",
        path.display()
    ));
}

fn log_cargo_check_result(
    log_fn: &mut impl FnMut(&str),
    owner: &str,
    repo_name: &str,
    result: &str,
) {
    log_fn(&format!(
        "cargo check: リポジトリ={owner}/{repo_name} 結果={result}"
    ));
}

fn format_git_rev_parse_head_command(path: &Path) -> String {
    format!("git -C {} rev-parse HEAD", path.display())
}

fn format_git_ls_remote_main_command(owner: &str, repo_name: &str) -> String {
    format!("git ls-remote https://github.com/{owner}/{repo_name}.git refs/heads/main")
}

fn cargo_install_entry_matches_repo(key: &str, owner: &str, repo_name: &str) -> bool {
    let src = key.trim_end_matches(')');
    let repo_url = format!("git+https://github.com/{owner}/{repo_name}");
    src.contains(&format!("{repo_url}#")) || src.contains(&format!("{repo_url}.git#"))
}

/// Format a one-line comparison summary for cargo hash investigation logs.
///
/// - `remote_hash`: latest hash resolved from the GitHub remote repository's `main` branch
/// - `installed_hash`: HEAD resolved from the selected cargo checkout under `git/checkouts`
/// - `local_hash`: HEAD resolved from the local repository clone under `base_dir`, for
///   diagnostic logging only
///
/// Logging all three values together makes it easier to see which source diverges when
/// an unexpected hash is being observed in the field.
fn format_cargo_hash_summary(remote_hash: &str, installed_hash: &str, local_hash: &str) -> String {
    fn match_status(matches: bool) -> &'static str {
        if matches {
            "一致"
        } else {
            "不一致"
        }
    }

    fn format_match_status(label: &str, matches: bool) -> String {
        format!("{label}={matches} ({})", match_status(matches))
    }

    let remote_eq_installed = remote_hash == installed_hash;
    let installed_eq_local = installed_hash == local_hash;
    let remote_eq_local = remote_hash == local_hash;
    let remote_vs_installed =
        format_match_status("リモートと cargo install の一致", remote_eq_installed);
    let installed_vs_local =
        format_match_status("cargo install とローカルの一致", installed_eq_local);
    let remote_vs_local = format_match_status("リモートとローカルの一致", remote_eq_local);
    format!(
        "ハッシュ要約: リモートハッシュ={remote_hash}, cargo install ハッシュ={installed_hash}, ローカルハッシュ={local_hash}, {remote_vs_installed}, {installed_vs_local}, {remote_vs_local}",
    )
}

fn log_cargo_check_command_result(
    log_fn: &mut impl FnMut(&str),
    owner: &str,
    repo_name: &str,
    command: &str,
    output: &Output,
) {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    log_fn(&format!(
        "cargo check: リポジトリ={owner}/{repo_name} コマンド={command} 結果=status={} 標準出力={stdout:?} 標準エラー={stderr:?}",
        output.status
    ));
}

#[cfg(test)]
#[path = "github_local_cargo_tests.rs"]
mod tests;
