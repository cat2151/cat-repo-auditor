use super::*;

// ──────────────────────────────────────────────
// check_cargo_git_install_inner tests
// ──────────────────────────────────────────────

fn make_crates2_json(owner: &str, repo: &str, crate_name: &str) -> String {
    // Builds a minimal .crates2.json with one git-installed entry.
    // Key format: "crate_name version (git+https://github.com/owner/repo#HASH)"
    let key = format!(
        "{crate_name} 0.1.0 (git+https://github.com/{owner}/{repo}#0123456789abcdef0123456789abcdef01234567)"
    );
    format!(
        "{{\"installs\":{{\"{key}\":{{\"version_req\":null,\"bins\":[\"{crate_name}\"],\
\"features\":[],\"all_features\":false,\"no_default_features\":false,\
\"profile\":\"release\",\"target\":\"x86_64-unknown-linux-gnu\",\
\"rustc\":\"rustc 1.80.0\",\"deps\":[]}}}}}}}}"
    )
}

#[test]
fn cargo_install_none_when_crates2_missing() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_missing_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // No .crates2.json created → function returns None
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap());
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
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap());
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_none_when_checkouts_dir_missing() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_nocheckouts_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();
    // No git/checkouts/ directory → function returns None
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap());
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
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();
    // checkouts/ exists but has no 'myrepo*' directory
    std::fs::create_dir_all(checkouts.join("other-repo-abc123")).unwrap();
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap());
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
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();
    // Two directories starting with 'myrepo' → error case
    std::fs::create_dir_all(checkouts.join("myrepo-aaaa1111")).unwrap();
    std::fs::create_dir_all(checkouts.join("myrepo-bbbb2222")).unwrap();
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap());
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_none_when_checkout_subdir_missing() {
    let tmp = std::env::temp_dir()
        .join(format!("cargo_test_nosubdir_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    // Checkout base exists but is empty (no sub-directory)
    std::fs::create_dir_all(checkouts.join("myrepo-abc12345")).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();
    let result = check_cargo_git_install_inner("owner", "myrepo", "/nonexistent", tmp.to_str().unwrap());
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
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
