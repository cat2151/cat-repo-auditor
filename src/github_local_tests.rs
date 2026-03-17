use super::*;

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
