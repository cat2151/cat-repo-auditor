use std::process::Command;

const OWNER_REPO: &str = "cat2151/cat-repo-auditor";

/// Pure decision function: returns true if `remote_hash` differs from `build_hash`
/// and both are non-empty and `build_hash` is not "unknown".
pub(crate) fn is_update_available(build_hash: &str, remote_hash: &str) -> bool {
    !build_hash.is_empty()
        && build_hash != "unknown"
        && !remote_hash.is_empty()
        && remote_hash != build_hash
}

/// Check if a newer version of cat-repo-auditor is available by comparing
/// the build-time commit hash (embedded at compile time) against the
/// latest commit on the remote repository's main branch.
/// Returns Some("owner/repo") if an update is available, None if up-to-date
/// or if the check cannot be performed.
pub fn check_self_update() -> Option<String> {
    let build_hash = env!("BUILD_COMMIT_HASH");

    // Get remote main branch HEAD commit hash via gh api
    let endpoint = format!("/repos/{OWNER_REPO}/commits/main");
    let out = Command::new("gh")
        .args(["api", &endpoint, "--jq", ".sha"])
        .output().ok()?;
    if !out.status.success() { return None; }
    let remote_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

    if is_update_available(build_hash, &remote_hash) {
        Some(OWNER_REPO.to_string())
    } else {
        None
    }
}

#[cfg(test)]
#[path = "self_update_tests.rs"]
mod tests;
