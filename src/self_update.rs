use cat_self_update_lib::{check_remote_commit, self_update as launch_self_update};
use std::sync::OnceLock;

pub(crate) const REPO_OWNER: &str = "cat2151";
pub(crate) const REPO_NAME: &str = "cat-repo-auditor";
const MAIN_BRANCH: &str = "main";
const INSTALL_CRATES: &[&str] = &[];

pub(crate) fn build_commit_hash() -> &'static str {
    env!("BUILD_COMMIT_HASH")
}

/// Full `cargo install` command string used in printed output.
///
/// This is shared by the self-update command output and the exit-time
/// update notice so both surfaces stay in sync.
pub(crate) fn install_cmd() -> String {
    format!("cargo install --force --git {}", git_url())
}

pub(crate) fn owner_repo() -> &'static str {
    static OWNER_REPO: OnceLock<String> = OnceLock::new();
    OWNER_REPO
        .get_or_init(|| format!("{REPO_OWNER}/{REPO_NAME}"))
        .as_str()
}

fn git_url() -> &'static str {
    static GIT_URL: OnceLock<String> = OnceLock::new();
    GIT_URL
        .get_or_init(|| format!("https://github.com/{}", owner_repo()))
        .as_str()
}

/// Perform a self-update.
pub fn run_self_update() -> anyhow::Result<bool> {
    launch_self_update(REPO_OWNER, REPO_NAME, INSTALL_CRATES)
        .map_err(|err| anyhow::anyhow!("failed to launch self-update helper: {err}"))?;
    println!("Running: {}", install_cmd());
    println!("The application will now exit so the updater can replace the binary.");
    println!("Restart catrepo after the update finishes.");
    Ok(true)
}

pub fn run_self_check() -> anyhow::Result<()> {
    let result = check_remote_commit(REPO_OWNER, REPO_NAME, MAIN_BRANCH, build_commit_hash())
        .map_err(|err| anyhow::anyhow!("failed to compare embedded vs remote commit: {err}"))?;
    println!("{result}");
    Ok(())
}

#[cfg(test)]
#[path = "self_update_tests.rs"]
mod tests;
