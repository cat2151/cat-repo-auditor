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
use hash::{cargo_install_source_hash, check_cargo_git_install_inner};

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

/// Format a one-line comparison summary for cargo hash investigation logs.
///
/// - `metadata_hash`: hash embedded in the matching `.crates2.json` install entry
/// - `installed_hash`: HEAD resolved from the selected cargo checkout under `git/checkouts`
/// - `local_hash`: HEAD resolved from the local repository clone under `base_dir`
///
/// Logging all three values together makes it easier to see which source diverges when
/// an unexpected hash is being observed in the field.
fn format_cargo_hash_summary(metadata_hash: &str, installed_hash: &str, local_hash: &str) -> String {
    format!(
        "hash summary: metadata={metadata_hash} installed={installed_hash} local={local_hash} metadata_eq_installed={} installed_eq_local={} metadata_eq_local={}",
        metadata_hash == installed_hash,
        installed_hash == local_hash,
        metadata_hash == local_hash,
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
