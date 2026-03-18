fn main() {
    // Embed the git commit hash at build time.
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o) } else { None })
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_COMMIT_HASH={hash}");

    // Re-run when HEAD switches branch (e.g. `git checkout`).
    println!("cargo:rerun-if-changed=.git/HEAD");

    // Re-run when the current branch ref advances (new commit).
    // Read .git/HEAD to discover the active branch ref path dynamically.
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        let head = head.trim();
        if let Some(ref_path) = head.strip_prefix("ref: ") {
            println!("cargo:rerun-if-changed=.git/{ref_path}");
        }
    }

    // Re-run when refs are packed (git gc or fetch can move commits here).
    println!("cargo:rerun-if-changed=.git/packed-refs");
}
