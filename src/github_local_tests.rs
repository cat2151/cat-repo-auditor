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

    // Sleep long enough for filesystems with 1-second mtime resolution to register
    // a distinct timestamp for the second directory.
    std::thread::sleep(std::time::Duration::from_millis(1100));

    // Create "new" sub-directory after a small delay (lexicographically first: "aaanew1")
    // Its mtime will be newer even though its name sorts earlier.
    let new_sub = checkouts.join("aaanew1");
    let expected_installed_hash = init_git_repo_with_content(&new_sub, "new-content");

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
