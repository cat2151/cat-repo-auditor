use std::process::Command;

const OWNER_REPO: &str = "cat2151/cat-repo-auditor";
const INSTALL_CMD: &str =
    "cargo install --force --git https://github.com/cat2151/cat-repo-auditor";

/// Pure decision function: returns true if `remote_hash` differs from `build_hash`
/// and both are non-empty and `build_hash` is not "unknown".
pub(crate) fn is_update_available(build_hash: &str, remote_hash: &str) -> bool {
    !build_hash.is_empty()
        && build_hash != "unknown"
        && !remote_hash.is_empty()
        && remote_hash != build_hash
}

/// Returns the content of the Windows bat file used to run self-update.
/// The bat waits a few seconds for the launching process to release the file
/// lock, runs `cargo install`, then deletes itself.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn update_bat_content() -> String {
    format!(
        "@echo off\r\ntimeout /t 3 /nobreak >nul\r\n{INSTALL_CMD}\r\ndel \"%~f0\"\r\n"
    )
}

/// Perform a self-update.
///
/// * **Windows** – writes a temporary `.bat` file, launches it detached (so
///   the OS file-lock on the running `.exe` is released before `cargo install`
///   overwrites it), then returns `Ok(true)` to signal that the caller should
///   exit immediately.
/// * **Other platforms** – runs `cargo install` in the foreground and returns
///   `Ok(false)`.
pub fn run_self_update() -> anyhow::Result<bool> {
    #[cfg(target_os = "windows")]
    {
        use std::io::Write;

        let bat_path = std::env::temp_dir().join("catrepo_update.bat");
        {
            let mut f = std::fs::File::create(&bat_path)?;
            f.write_all(update_bat_content().as_bytes())?;
        }

        // Launch the bat file detached so it outlives this process.
        let bat_str = bat_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("temp bat path is not valid UTF-8"))?;
        Command::new("cmd")
            .args(["/C", "start", "", bat_str])
            .spawn()?;

        println!("Launching update script: {}", bat_path.display());
        println!("The application will now exit so the file lock is released.");
        return Ok(true);
    }

    #[cfg(not(target_os = "windows"))]
    {
        println!("Running: {INSTALL_CMD}");
        let status = Command::new("cargo")
            .args(["install", "--force", "--git",
                   "https://github.com/cat2151/cat-repo-auditor"])
            .status()?;
        if !status.success() {
            anyhow::bail!("cargo install failed with status: {status}");
        }
        Ok(false)
    }
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
