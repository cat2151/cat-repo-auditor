use std::io::Write;
use std::path::Path;
use std::process::Output;

#[path = "github_local_cargo_bins.rs"]
mod bins;
#[path = "github_local_cargo_hash.rs"]
mod hash;

pub(crate) use bins::get_cargo_bins;
pub(crate) use hash::check_cargo_git_install;

#[cfg(test)]
use bins::get_cargo_bins_inner;
#[cfg(test)]
use hash::{check_cargo_git_install_inner, check_cargo_git_install_inner_with_remote_hash};

/// Returns the effective CARGO_HOME path.
fn get_cargo_home() -> String {
    std::env::var("CARGO_HOME").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();
        format!("{home}/.cargo")
    })
}

/// Append timestamped log messages to the unified local log file.
fn append_log_messages(messages: impl IntoIterator<Item = impl AsRef<str>>) {
    let log_path = crate::config::Config::log_path();
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        for msg in messages {
            let _ = writeln!(f, "[{now}] {}", msg.as_ref());
        }
    }
}

fn append_log_message(msg: &str) {
    append_log_messages(std::iter::once(msg));
}

pub(crate) fn append_cargo_check_results(owner: &str, results: &[(String, String)]) {
    append_log_messages(results.iter().map(|(repo_name, result)| {
        format!("cargo check: repo={owner}/{repo_name} result={result}")
    }));
}

fn log_cargo_check_path_result(
    log_fn: &mut impl FnMut(&str),
    owner: &str,
    repo_name: &str,
    path: &Path,
    result: &str,
) {
    log_fn(&format!(
        "cargo check: repo={owner}/{repo_name} path={} result={result}",
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
        "cargo check: repo={owner}/{repo_name} result={result}"
    ));
}

fn format_git_rev_parse_head_command(path: &Path) -> String {
    format!("git -C {} rev-parse HEAD", path.display())
}

fn format_git_ls_remote_main_command(owner: &str, repo_name: &str) -> String {
    format!("git ls-remote https://github.com/{owner}/{repo_name}.git refs/heads/main")
}

/// Format a one-line comparison summary for cargo hash investigation logs.
///
/// - `remote_hash`: latest hash resolved from the GitHub remote repository's `main` branch
/// - `installed_hash`: HEAD resolved from the selected cargo checkout under `git/checkouts`
/// - `local_hash`: HEAD resolved from the local repository clone under `base_dir`
///
/// Logging all three values together makes it easier to see which source diverges when
/// an unexpected hash is being observed in the field.
fn format_cargo_hash_summary(remote_hash: &str, installed_hash: &str, local_hash: &str) -> String {
    fn match_status(matches: bool) -> &'static str {
        if matches {
            "match"
        } else {
            "mismatch"
        }
    }

    fn format_match_status(label: &str, matches: bool) -> String {
        format!("{label}={matches} ({})", match_status(matches))
    }

    let remote_eq_installed = remote_hash == installed_hash;
    let installed_eq_local = installed_hash == local_hash;
    let remote_eq_local = remote_hash == local_hash;
    let remote_vs_installed = format_match_status("remote_eq_installed", remote_eq_installed);
    let installed_vs_local = format_match_status("installed_eq_local", installed_eq_local);
    let remote_vs_local = format_match_status("remote_eq_local", remote_eq_local);
    format!(
        "hash summary: remote hash={remote_hash} installed hash={installed_hash} local hash={local_hash} {remote_vs_installed} {installed_vs_local} {remote_vs_local}",
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
        "cargo check: repo={owner}/{repo_name} command={command} result=status={} stdout={stdout:?} stderr={stderr:?}",
        output.status
    ));
}

#[cfg(test)]
#[path = "github_local_cargo_tests.rs"]
mod tests;
