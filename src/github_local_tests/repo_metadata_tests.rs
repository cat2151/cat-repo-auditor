use super::helpers::*;
use super::*;

#[test]
fn check_deepwiki_exists_finds_link_in_readme_ja() {
    let tmp = std::env::temp_dir().join(format!("deepwiki_test_a_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("README.ja.md"),
        "See https://deepwiki.com/owner/repo\n",
    )
    .unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_deepwiki_exists_false_when_no_link() {
    let tmp = std::env::temp_dir().join(format!("deepwiki_test_b_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.ja.md"), "No links here\n").unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_deepwiki_exists_falls_back_to_readme_md() {
    let tmp = std::env::temp_dir().join(format!("deepwiki_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("README.md"),
        "See https://deepwiki.com/owner/repo\n",
    )
    .unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_deepwiki_exists_false_when_no_files() {
    let tmp = std::env::temp_dir().join(format!("deepwiki_test_d_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    let result = check_deepwiki_exists(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_all_present_returns_true() {
    let tmp = std::env::temp_dir().join(format!("wf_test_a_{}", std::process::id()));
    let wf_dir = tmp.join("myrepo").join(".github").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    for f in &[
        "call-translate-readme.yml",
        "call-issue-note.yml",
        "call-check-large-files.yml",
    ] {
        std::fs::write(wf_dir.join(f), "").unwrap();
    }
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo", None);
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn check_workflows_missing_one_returns_false() {
    let tmp = std::env::temp_dir().join(format!("wf_test_b_{}", std::process::id()));
    let wf_dir = tmp.join("myrepo").join(".github").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    for f in &["call-translate-readme.yml", "call-issue-note.yml"] {
        std::fs::write(wf_dir.join(f), "").unwrap();
    }
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo", None);
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_empty_dir_returns_false() {
    let tmp = std::env::temp_dir().join(format!("wf_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo", None);
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_cargo_requires_check_workflow() {
    let tmp = std::env::temp_dir().join(format!("wf_test_d_{}", std::process::id()));
    let wf_dir = tmp.join("myrepo").join(".github").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    for f in &[
        "call-translate-readme.yml",
        "call-issue-note.yml",
        "call-check-large-files.yml",
    ] {
        std::fs::write(wf_dir.join(f), "").unwrap();
    }
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo", Some(true));
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_stale_cargo_with_check_workflow() {
    let tmp = std::env::temp_dir().join(format!("wf_test_e_{}", std::process::id()));
    let wf_dir = tmp.join("myrepo").join(".github").join("workflows");
    std::fs::create_dir_all(&wf_dir).unwrap();
    for f in &[
        "call-translate-readme.yml",
        "call-issue-note.yml",
        "call-check-large-files.yml",
        "call-rust-windows-cargo-check.yml",
    ] {
        std::fs::write(wf_dir.join(f), "").unwrap();
    }
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo", Some(false));
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}

#[test]
fn collect_workflow_repo_exist_checks_groups_installed_and_missing_repos() {
    let tmp = unique_temp_dir("wf_repo_exist");
    let _tmp_guard = TempDirGuard::new(tmp.clone());
    let source_wf_dir = tmp
        .join(WORKFLOW_SOURCE_REPO)
        .join(".github")
        .join("workflows");
    std::fs::create_dir_all(&source_wf_dir).unwrap();
    std::fs::write(source_wf_dir.join("call-a.yml"), "name: a\n").unwrap();
    std::fs::write(source_wf_dir.join("call-b.yml"), "name: b\n").unwrap();
    std::fs::write(source_wf_dir.join("callg-bad.yml"), "ignore\n").unwrap();
    std::fs::write(source_wf_dir.join("note.txt"), "ignore\n").unwrap();

    let repo_a = tmp.join("repo-a").join(".github").join("workflows");
    let repo_b = tmp.join("repo-b").join(".github").join("workflows");
    std::fs::create_dir_all(&repo_a).unwrap();
    std::fs::create_dir_all(&repo_b).unwrap();
    std::fs::write(repo_a.join("call-a.yml"), "repo-a\n").unwrap();
    std::fs::write(repo_b.join("call-a.yml"), "repo-b\n").unwrap();
    std::fs::write(repo_b.join("call-b.yml"), "repo-b\n").unwrap();

    let checks = collect_workflow_repo_exist_checks(
        tmp.to_str().unwrap(),
        &[
            make_repo("repo-b", "today", "2026-03-28T00:00:00Z"),
            make_repo(WORKFLOW_SOURCE_REPO, "3d", "2026-03-25T00:00:00Z"),
            make_repo("repo-a", "2w", "2026-03-14T00:00:00Z"),
        ],
    )
    .unwrap();

    assert_eq!(checks.len(), 2);
    assert_eq!(checks[0].workflow_file, "call-a.yml");
    assert_eq!(
        checks[0].installed_repos,
        vec![
            WorkflowRepoExistRepo {
                name: String::from("repo-b"),
                updated_at: String::from("today"),
                updated_at_raw: String::from("2026-03-28T00:00:00Z"),
            },
            WorkflowRepoExistRepo {
                name: String::from("repo-a"),
                updated_at: String::from("2w"),
                updated_at_raw: String::from("2026-03-14T00:00:00Z"),
            },
        ]
    );
    assert!(checks[0].missing_repos.is_empty());
    assert_eq!(checks[1].workflow_file, "call-b.yml");
    assert_eq!(
        checks[1].installed_repos,
        vec![WorkflowRepoExistRepo {
            name: String::from("repo-b"),
            updated_at: String::from("today"),
            updated_at_raw: String::from("2026-03-28T00:00:00Z"),
        }]
    );
    assert_eq!(
        checks[1].missing_repos,
        vec![WorkflowRepoExistRepo {
            name: String::from("repo-a"),
            updated_at: String::from("2w"),
            updated_at_raw: String::from("2026-03-14T00:00:00Z"),
        }]
    );
}

#[test]
fn check_readme_ja_badge_finds_self_reference() {
    let tmp = std::env::temp_dir().join(format!("badge_test_a_{}", std::process::id()));
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
    let tmp = std::env::temp_dir().join(format!("badge_test_b_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.ja.md"), "No badge here\n").unwrap();
    let result = check_readme_ja_badge(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_readme_ja_badge_falls_back_to_readme_md() {
    let tmp = std::env::temp_dir().join(format!("badge_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("README.md"), "See README.ja.md\n").unwrap();
    let result = check_readme_ja_badge(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result);
}
