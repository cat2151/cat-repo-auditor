use super::*;
use std::process::Command as Cmd;

// ──────────────────────────────────────────────
// check_cargo_git_install_inner tests
// ──────────────────────────────────────────────

fn make_crates2_json(owner: &str, repo: &str, crate_name: &str) -> String {
    // Builds a minimal .crates2.json with one git-installed entry.
    // Key format: "crate_name version (git+https://github.com/owner/repo#HASH)"
    let key = format!(
        "{crate_name} 0.1.0 (git+https://github.com/{owner}/{repo}#0123456789abcdef0123456789abcdef01234567)"
    );
    // 3 opening braces → 3 closing braces: outer obj, installs obj, value obj
    format!(
        "{{\"installs\":{{\"{key}\":{{\"version_req\":null,\"bins\":[\"{crate_name}\"],\
\"features\":[],\"all_features\":false,\"no_default_features\":false,\
\"profile\":\"release\",\"target\":\"x86_64-unknown-linux-gnu\",\
\"rustc\":\"rustc 1.80.0\",\"deps\":[]}}}}}}"
    )
}

/// Create a minimal git repo at `path` with one commit using `content`; return the HEAD hash.
fn init_git_repo_with_content(path: &std::path::Path, content: &str) -> String {
    std::fs::create_dir_all(path).unwrap();
    let run = |args: &[&str]| {
        let out = Cmd::new("git").args(args).current_dir(path).output()
            .unwrap_or_else(|e| panic!("git {:?} spawn failed: {e}", args));
        assert!(out.status.success(), "git {:?} failed: {}", args, String::from_utf8_lossy(&out.stderr));
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

/// Create a minimal git repo at `path` with one commit; return the HEAD hash.
fn init_git_repo(path: &std::path::Path) -> String {
    init_git_repo_with_content(path, "content-a")
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir()
        .join(format!("{prefix}_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_git_ok(path: &std::path::Path, args: &[&str]) -> std::process::Output {
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

fn setup_remote_with_clone(test_name: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let tmp = unique_temp_dir(test_name);
    let remote = tmp.join("remote.git");
    let seed = tmp.join("seed");
    let base = tmp.join("repos");
    std::fs::create_dir_all(&base).unwrap();

    let init_out = Cmd::new("git")
        .args(["init", "--bare", remote.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(init_out.status.success(), "git init --bare failed: {}", String::from_utf8_lossy(&init_out.stderr));

    std::fs::create_dir_all(&seed).unwrap();
    run_git_ok(&tmp, &["clone", remote.to_str().unwrap(), seed.to_str().unwrap()]);
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

#[test]
fn cargo_install_none_when_crates2_missing() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_missing_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // No .crates2.json created → function returns None
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap(), |_| {});
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn check_local_status_reports_modified_before_pullable() {
    let tmp = unique_temp_dir("status_modified");
    let repo = tmp.join("myrepo");
    init_git_repo(&repo);
    std::fs::write(repo.join("f"), "changed-but-unstaged").unwrap();

    let (status, has_local_git, files) = check_local_status_no_fetch(tmp.to_str().unwrap(), "myrepo");

    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status, LocalStatus::Modified);
    assert!(has_local_git);
    assert!(!files.is_empty());
}

#[test]
fn check_local_status_reports_staging_before_pullable() {
    let tmp = unique_temp_dir("status_staging");
    let repo = tmp.join("myrepo");
    init_git_repo(&repo);
    std::fs::write(repo.join("f"), "changed-and-staged").unwrap();
    run_git_ok(&repo, &["add", "f"]);

    let (status, has_local_git, files) = check_local_status_no_fetch(tmp.to_str().unwrap(), "myrepo");

    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status, LocalStatus::Staging);
    assert!(has_local_git);
    assert!(!files.is_empty());
}

#[test]
fn check_local_status_reports_conflict() {
    let tmp = unique_temp_dir("status_conflict");
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    run_git_ok(&repo, &["init", "-b", "master"]);
    run_git_ok(&repo, &["config", "user.email", "t@t.com"]);
    run_git_ok(&repo, &["config", "user.name", "T"]);
    std::fs::write(repo.join("f"), "base\n").unwrap();
    run_git_ok(&repo, &["add", "f"]);
    run_git_ok(&repo, &["commit", "-m", "base"]);
    run_git_ok(&repo, &["checkout", "-b", "feature"]);
    std::fs::write(repo.join("f"), "feature\n").unwrap();
    run_git_ok(&repo, &["commit", "-am", "feature"]);
    run_git_ok(&repo, &["checkout", "master"]);
    std::fs::write(repo.join("f"), "master\n").unwrap();
    run_git_ok(&repo, &["commit", "-am", "master"]);

    let merge = Cmd::new("git")
        .args(["merge", "feature"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(!merge.status.success(), "merge unexpectedly succeeded");

    let (status, has_local_git, files) = check_local_status_no_fetch(tmp.to_str().unwrap(), "myrepo");

    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status, LocalStatus::Conflict);
    assert!(has_local_git);
    assert!(files.iter().any(|line| line.starts_with("UU ")));
}

#[test]
fn git_pull_stashes_modified_changes_and_restores_them() {
    let (tmp, seed, local) = setup_remote_with_clone("pull_modified");

    std::fs::write(local.join("local-only.txt"), "local change\n").unwrap();
    std::fs::write(seed.join("remote-only.txt"), "remote change\n").unwrap();
    run_git_ok(&seed, &["add", "remote-only.txt"]);
    run_git_ok(&seed, &["commit", "-m", "remote update"]);
    run_git_ok(&seed, &["push", "origin", "HEAD"]);

    let (status_before, _, _) = check_local_status_no_fetch(
        tmp.join("repos").to_str().unwrap(),
        "myrepo",
    );
    assert_eq!(status_before, LocalStatus::Modified);

    git_pull(tmp.join("repos").to_str().unwrap(), "myrepo").unwrap();

    assert!(local.join("local-only.txt").exists());
    assert!(local.join("remote-only.txt").exists());
    let (status_after, _, files_after) = check_local_status_no_fetch(
        tmp.join("repos").to_str().unwrap(),
        "myrepo",
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status_after, LocalStatus::Modified);
    assert!(files_after.iter().any(|line| line.contains("local-only.txt")));
}

#[test]
fn git_pull_stashes_staged_changes_and_restores_them() {
    let (tmp, seed, local) = setup_remote_with_clone("pull_staging");

    std::fs::write(local.join("staged.txt"), "staged change\n").unwrap();
    run_git_ok(&local, &["add", "staged.txt"]);
    std::fs::write(seed.join("remote-only.txt"), "remote change\n").unwrap();
    run_git_ok(&seed, &["add", "remote-only.txt"]);
    run_git_ok(&seed, &["commit", "-m", "remote update"]);
    run_git_ok(&seed, &["push", "origin", "HEAD"]);

    let (status_before, _, _) = check_local_status_no_fetch(
        tmp.join("repos").to_str().unwrap(),
        "myrepo",
    );
    assert_eq!(status_before, LocalStatus::Staging);

    git_pull(tmp.join("repos").to_str().unwrap(), "myrepo").unwrap();

    assert!(local.join("staged.txt").exists());
    assert!(local.join("remote-only.txt").exists());
    let (status_after, _, files_after) = check_local_status_no_fetch(
        tmp.join("repos").to_str().unwrap(),
        "myrepo",
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status_after, LocalStatus::Staging);
    assert!(files_after.iter().any(|line| line.contains("staged.txt")));
}

#[test]
fn git_pull_marks_repo_conflict_when_stash_pop_conflicts() {
    let (tmp, seed, local) = setup_remote_with_clone("pull_conflict");

    std::fs::write(local.join("file.txt"), "local change\n").unwrap();
    std::fs::write(seed.join("file.txt"), "remote change\n").unwrap();
    run_git_ok(&seed, &["commit", "-am", "remote conflict"]);
    run_git_ok(&seed, &["push", "origin", "HEAD"]);

    let err = git_pull(tmp.join("repos").to_str().unwrap(), "myrepo").unwrap_err();
    assert!(
        err.to_string().contains("conflict") || err.to_string().contains("Merge conflict"),
        "unexpected error: {err:#}"
    );

    let (status_after, _, files_after) = check_local_status_no_fetch(
        tmp.join("repos").to_str().unwrap(),
        "myrepo",
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status_after, LocalStatus::Conflict);
    assert!(files_after.iter().any(|line| line.starts_with("UU ")));
}

#[test]
fn cargo_install_none_when_repo_not_in_crates2() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_notfound_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // crates2.json for a different repo
    let json = make_crates2_json("other", "other-repo", "other-repo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    // Verify JSON is valid before running the test
    assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok(), "make_crates2_json produced invalid JSON");
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap(), |_| {});
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_none_when_checkouts_dir_missing() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_nocheckouts_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    // No git/checkouts/ directory → function returns None
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap(), |_| {});
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_none_when_no_matching_checkout_dir() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_nomatch_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    // checkouts/ exists but has no 'myrepo-*' directory
    std::fs::create_dir_all(checkouts.join("other-repo-abc123")).unwrap();
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap(), |_| {});
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_prefix_does_not_match_longer_crate_name() {
    // "myrepo" should not match "myrepo-extra-abc123"
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_prefix_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    // "myrepo-extra-*" should NOT match a search for crate "myrepo"
    std::fs::create_dir_all(checkouts.join("myrepo-extra-abc123")).unwrap();
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap(), |_| {});
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_none_when_multiple_checkout_dirs_match() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_multi_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    // Two directories starting with 'myrepo-' → multiple-match error case
    std::fs::create_dir_all(checkouts.join("myrepo-aaaa1111")).unwrap();
    std::fs::create_dir_all(checkouts.join("myrepo-bbbb2222")).unwrap();
    let mut logged = String::new();
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap(), |msg| {
        logged = msg.to_string();
    });
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
    assert!(logged.contains("multiple checkouts"), "log_fn should be called with error message");
}

#[test]
fn cargo_install_none_when_checkout_subdir_missing() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_nosubdir_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    // Checkout base exists but is empty (no sub-directory)
    std::fs::create_dir_all(checkouts.join("myrepo-abc12345")).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap(), |_| {});
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_returns_some_true_when_hashes_match() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_match_{}", std::process::id()));

    // Set up local git repo
    let local_repo = tmp.join("repos").join("myrepo");
    let local_hash = init_git_repo(&local_repo);

    // Clone the local repo into the cargo checkout sub-directory so both have the same HEAD.
    let cargo_home = tmp.join("cargo_home");
    let installed_sub = cargo_home.join("git").join("checkouts")
        .join("myrepo-xyz99999").join("head1234");
    let out = Cmd::new("git")
        .args(["clone", "--local", local_repo.to_str().unwrap(), installed_sub.to_str().unwrap()])
        .output().unwrap();
    assert!(out.status.success(), "git clone failed: {}", String::from_utf8_lossy(&out.stderr));

    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();

    let result = check_cargo_git_install_inner(
        "owner", "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc) = result.expect("should return Some");
    assert!(matches, "hashes should match: inst={inst} loc={loc}");
    assert_eq!(inst, local_hash);
    assert_eq!(loc, local_hash);
}

#[test]
fn cargo_install_logs_hash_source_details() {
    let tmp = unique_temp_dir("cargo_test_hash_log");

    let local_repo_path = tmp.join("repos").join("myrepo");
    init_git_repo(&local_repo_path);

    let cargo_home = tmp.join("cargo_home");
    let installed_checkout_path = cargo_home.join("git").join("checkouts")
        .join("myrepo-xyz99999").join("head1234");
    let out = Cmd::new("git")
        .args(["clone", "--local", local_repo_path.to_str().unwrap(), installed_checkout_path.to_str().unwrap()])
        .output().unwrap();
    assert!(out.status.success(), "git clone failed: {}", String::from_utf8_lossy(&out.stderr));

    let crates2_path = cargo_home.join(".crates2.json");
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(&crates2_path, &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner(
        "owner", "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_some(), "should return Some");
    assert!(logs.iter().any(|msg| msg.contains(crates2_path.to_str().unwrap())),
        "log should contain crates2.json path: {logs:?}");
    assert!(logs.iter().any(|msg| msg.contains(installed_checkout_path.to_str().unwrap())),
        "log should contain installed checkout dir: {logs:?}");
    assert!(logs.iter().any(|msg| msg.contains("git -C") && msg.contains("rev-parse HEAD")),
        "log should contain installed hash source command: {logs:?}");
}

#[test]
fn cargo_install_returns_some_false_when_hashes_differ() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_stale_{}", std::process::id()));

    // Set up local git repo with one commit
    let local_repo = tmp.join("repos").join("myrepo");
    let local_hash = init_git_repo_with_content(&local_repo, "local-content");

    // Set up cargo checkout git repo with a different commit (different content)
    let sub_dir = tmp.join("cargo_home").join("git").join("checkouts")
        .join("myrepo-abc12345").join("abcdef12");
    let installed_hash = init_git_repo_with_content(&sub_dir, "installed-content");

    // Ensure hashes differ (different content → different tree → different hash)
    assert_ne!(local_hash, installed_hash, "test setup: repos should have different hashes");

    let json = make_crates2_json("owner", "myrepo", "myrepo");
    let cargo_home = tmp.join("cargo_home");
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();

    let result = check_cargo_git_install_inner(
        "owner", "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc) = result.expect("should return Some");
    assert!(!matches, "hashes should differ: inst={inst} loc={loc}");
    assert_eq!(inst, installed_hash);
    assert_eq!(loc, local_hash);
}

#[test]
fn cargo_install_picks_latest_mtime_subdir() {
    // When there are multiple sub-directories under the checkout base, the function
    // must use the one whose modification timestamp is the most recent, not the one
    // that comes last in lexicographic order.
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_mtime_{}", std::process::id()));

    // Set up local git repo
    let local_repo = tmp.join("repos").join("myrepo");
    let local_hash = init_git_repo_with_content(&local_repo, "local-content");

    let cargo_home = tmp.join("cargo_home");
    let checkouts = cargo_home.join("git").join("checkouts").join("myrepo-abc12345");

    // Create "old" sub-directory first (lexicographically last: "zzzzold1")
    let old_sub = checkouts.join("zzzzold1");
    init_git_repo_with_content(&old_sub, "old-content");

    // Create "new" sub-directory (lexicographically first: "aaanew1")
    let new_sub = checkouts.join("aaanew1");
    let expected_installed_hash = init_git_repo_with_content(&new_sub, "new-content");

    // Explicitly set directory mtimes to deterministic values so the test is
    // reliable even on filesystems with coarse (1 s) timestamp resolution, and
    // so the suite does not need to sleep.
    {
        let f = std::fs::File::open(&old_sub).unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
        )).unwrap();
    }
    {
        let f = std::fs::File::open(&new_sub).unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(2_000_000),
        )).unwrap();
    }

    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();

    let result = check_cargo_git_install_inner(
        "owner", "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();

    // The result should be based on "aaanew1" (newest mtime), not "zzzzold1"
    let (_matches, inst, _loc) = result.expect("should return Some");
    assert_eq!(inst, expected_installed_hash,
        "should have picked the subdir with the latest mtime (aaanew1), not zzzzold1");
    assert_ne!(inst, local_hash);
}

#[test]
fn check_deepwiki_exists_finds_link_in_readme_ja() {
    let tmp = std::env::temp_dir()
        .join(format!("deepwiki_test_a_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.ja.md"), "See https://deepwiki.com/owner/repo\n").unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_deepwiki_exists_false_when_no_link() {
    let tmp = std::env::temp_dir()
        .join(format!("deepwiki_test_b_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.ja.md"), "No links here\n").unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_deepwiki_exists_falls_back_to_readme_md() {
    let tmp = std::env::temp_dir()
        .join(format!("deepwiki_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.md"), "See https://deepwiki.com/owner/repo\n").unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_deepwiki_exists_false_when_no_files() {
    let tmp = std::env::temp_dir()
        .join(format!("deepwiki_test_d_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_all_present_returns_true() {
    let tmp = std::env::temp_dir()
        .join(format!("wf_test_a_{}", std::process::id()));
    let wf_dir = tmp.join("myrepo").join(".github").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    for f in &[
        "call-translate-readme.yml",
        "call-issue-note.yml",
        "call-check-large-files.yml",
    ] {
        std::fs::write(wf_dir.join(f), "").unwrap();
    }
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_workflows_missing_one_returns_false() {
    let tmp = std::env::temp_dir()
        .join(format!("wf_test_b_{}", std::process::id()));
    let wf_dir = tmp.join("myrepo").join(".github").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    for f in &["call-translate-readme.yml", "call-issue-note.yml"] {
        std::fs::write(wf_dir.join(f), "").unwrap();
    }
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_empty_dir_returns_false() {
    let tmp = std::env::temp_dir()
        .join(format!("wf_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_readme_ja_badge_finds_self_reference() {
    let tmp = std::env::temp_dir()
        .join(format!("badge_test_a_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("README.ja.md"),
        "[![ja](README.ja.md)](README.ja.md)\n",
    )
    .unwrap();
    let result = check_readme_ja_badge(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_readme_ja_badge_false_when_no_self_reference() {
    let tmp = std::env::temp_dir()
        .join(format!("badge_test_b_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.ja.md"), "No badge here\n").unwrap();
    let result = check_readme_ja_badge(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_readme_ja_badge_falls_back_to_readme_md() {
    let tmp = std::env::temp_dir()
        .join(format!("badge_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.md"), "See README.ja.md\n").unwrap();
    let result = check_readme_ja_badge(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}
