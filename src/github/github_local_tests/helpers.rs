use crate::github::{LocalStatus, RepoInfo};
use std::process::Command as Cmd;

pub(super) fn init_git_repo_with_content(path: &std::path::Path, content: &str) -> String {
    std::fs::create_dir_all(path).unwrap();
    let run = |args: &[&str]| {
        let out = Cmd::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .unwrap_or_else(|e| panic!("git {:?} spawn failed: {e}", args));
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    run(&["init"]);
    run(&["config", "user.email", "t@t.com"]);
    run(&["config", "user.name", "T"]);
    std::fs::write(path.join("f"), content).unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "init"]);
    let out = run(&["rev-parse", "HEAD"]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub(super) fn init_git_repo(path: &std::path::Path) -> String {
    init_git_repo_with_content(path, "content-a")
}

pub(super) fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) struct TempDirGuard {
    path: std::path::PathBuf,
}

impl TempDirGuard {
    pub(super) fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub(super) fn run_git_ok(path: &std::path::Path, args: &[&str]) -> std::process::Output {
    let out = Cmd::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .unwrap_or_else(|e| panic!("git {:?} spawn failed: {e}", args));
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

pub(super) fn setup_remote_with_clone(
    test_name: &str,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let tmp = unique_temp_dir(test_name);
    let remote = tmp.join("remote.git");
    let seed = tmp.join("seed");
    let base = tmp.join("repos");
    std::fs::create_dir_all(&base).unwrap();

    let init_out = Cmd::new("git")
        .args(["init", "--bare", remote.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        init_out.status.success(),
        "git init --bare failed: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );

    std::fs::create_dir_all(&seed).unwrap();
    run_git_ok(
        &tmp,
        &["clone", remote.to_str().unwrap(), seed.to_str().unwrap()],
    );
    run_git_ok(&seed, &["config", "user.email", "t@t.com"]);
    run_git_ok(&seed, &["config", "user.name", "T"]);
    std::fs::write(seed.join("file.txt"), "base\n").unwrap();
    run_git_ok(&seed, &["add", "file.txt"]);
    run_git_ok(&seed, &["commit", "-m", "initial"]);
    run_git_ok(&seed, &["push", "origin", "HEAD"]);

    run_git_ok(&base, &["clone", remote.to_str().unwrap(), "myrepo"]);
    let local = base.join("myrepo");
    run_git_ok(&local, &["config", "user.email", "t@t.com"]);
    run_git_ok(&local, &["config", "user.name", "T"]);

    (tmp, seed, local)
}

pub(super) fn make_repo(name: &str, updated_at: &str, updated_at_raw: &str) -> RepoInfo {
    RepoInfo {
        name: name.to_string(),
        full_name: format!("owner/{name}"),
        updated_at: updated_at.to_string(),
        updated_at_raw: updated_at_raw.to_string(),
        open_issues: 0,
        open_prs: 0,
        is_private: false,
        local_status: LocalStatus::Clean,
        has_local_git: true,
        staging_files: vec![],
        issues: vec![],
        prs: vec![],
        readme_ja: None,
        readme_ja_checked_at: String::new(),
        readme_ja_badge: None,
        readme_ja_badge_checked_at: String::new(),
        pages: None,
        pages_checked_at: String::new(),
        deepwiki: None,
        deepwiki_checked_at: String::new(),
        cargo_install: None,
        cargo_checked_at: String::new(),
        cargo_remote_hash: String::new(),
        cargo_remote_hash_checked_at: String::new(),
        cargo_installed_hash: String::new(),
        wf_workflows: None,
        wf_checked_at: String::new(),
    }
}
