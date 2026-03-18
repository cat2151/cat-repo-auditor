use std::process::Command;

const OWNER_REPO: &str = "cat2151/cat-repo-auditor";

/// Check if a newer version of cat-repo-auditor is available by comparing
/// the build-time commit hash (embedded at compile time) against the
/// latest commit on the remote repository's main branch.
/// Returns Some("owner/repo") if an update is available, None if up-to-date
/// or if the check cannot be performed.
pub fn check_self_update() -> Option<String> {
    let build_hash = env!("BUILD_COMMIT_HASH");
    if build_hash == "unknown" || build_hash.is_empty() { return None; }

    // Get remote main branch HEAD commit hash via gh api
    let endpoint = format!("/repos/{OWNER_REPO}/commits/main");
    let out = Command::new("gh")
        .args(["api", &endpoint, "--jq", ".sha"])
        .output().ok()?;
    if !out.status.success() { return None; }
    let remote_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

    if remote_hash.is_empty() || remote_hash == build_hash { return None; }
    Some(OWNER_REPO.to_string())
}
