use cat_self_update_lib::{check_remote_commit, self_update as launch_self_update};

const REPO_OWNER: &str = "cat2151";
const REPO_NAME: &str = "cat-repo-auditor";
const MAIN_BRANCH: &str = "main";
pub(crate) const OWNER_REPO: &str = "cat2151/cat-repo-auditor";
const GIT_URL: &str = "https://github.com/cat2151/cat-repo-auditor";
const BIN_NAMES: &[&str] = &["catrepo"];

pub(crate) fn build_commit_hash() -> &'static str {
    env!("BUILD_COMMIT_HASH")
}

/// Full `cargo install` command string used in printed output.
///
/// This is shared by the self-update command output and the exit-time
/// update notice so both surfaces stay in sync.
pub(crate) fn install_cmd() -> String {
    format!("cargo install --force --git {GIT_URL}")
}

/// Pure decision function: returns true if `remote_hash` differs from `build_hash`
/// and both are non-empty and `build_hash` is not "unknown".
pub(crate) fn is_update_available(build_hash: &str, remote_hash: &str) -> bool {
    !build_hash.is_empty()
        && build_hash != "unknown"
        && !remote_hash.is_empty()
        && remote_hash != build_hash
}

/// Perform a self-update.
pub fn run_self_update() -> anyhow::Result<bool> {
    launch_self_update(REPO_OWNER, REPO_NAME, BIN_NAMES)
        .map_err(|err| anyhow::anyhow!("failed to launch self-update helper: {err}"))?;
    println!("Running: {}", install_cmd());
    println!("The application will now exit so the updater can replace the binary.");
    Ok(true)
}

/// Check if a newer version of cat-repo-auditor is available by comparing
/// the build-time commit hash (embedded at compile time) against the
/// latest commit on the remote repository's main branch.
/// Returns Some("owner/repo") if an update is available, None if up-to-date
/// or if the check cannot be performed.
pub fn check_self_update() -> Option<String> {
    let build_hash = build_commit_hash();
    let result = check_remote_commit(REPO_OWNER, REPO_NAME, MAIN_BRANCH, build_hash).ok()?;

    if is_update_available(&result.embedded_hash, &result.remote_hash) {
        Some(OWNER_REPO.to_string())
    } else {
        None
    }
}

#[cfg(test)]
#[path = "self_update_tests.rs"]
mod tests;
