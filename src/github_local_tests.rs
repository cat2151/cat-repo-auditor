use super::*;
use std::process::Command as Cmd;

/// Create a minimal git repo at `path` with one commit using `content`; return the HEAD hash.
fn init_git_repo_with_content(path: &std::path::Path, content: &str) -> String {
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

/// Create a minimal git repo at `path` with one commit; return the HEAD hash.
fn init_git_repo(path: &std::path::Path) -> String {
    init_git_repo_with_content(path, "content-a")
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
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

fn setup_remote_with_clone(
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

#[test]
fn check_local_status_reports_modified_before_pullable() {
    let tmp = unique_temp_dir("status_modified");
    let repo = tmp.join("myrepo");
    init_git_repo(&repo);
    std::fs::write(repo.join("f"), "changed-but-unstaged").unwrap();

    let (status, has_local_git, files) =
        check_local_status_no_fetch(tmp.to_str().unwrap(), "myrepo");

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

    let (status, has_local_git, files) =
        check_local_status_no_fetch(tmp.to_str().unwrap(), "myrepo");

    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status, LocalStatus::Staging);
    assert!(has_local_git);
    assert!(!files.is_empty());
}

#[test]
fn local_head_matches_upstream_returns_true_for_modified_repo_with_same_head() {
    let (tmp, _seed, local) = setup_remote_with_clone("same_head_modified");

    std::fs::write(local.join("local-only.txt"), "local change\n").unwrap();

    let (status, _, _) = check_local_status_no_fetch(tmp.join("repos").to_str().unwrap(), "myrepo");

    assert_eq!(status, LocalStatus::Modified);
    assert!(local_head_matches_upstream(
        tmp.join("repos").to_str().unwrap(),
        "myrepo"
    ));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn local_head_matches_upstream_returns_true_for_staging_repo_with_same_head() {
    let (tmp, _seed, local) = setup_remote_with_clone("same_head_staging");

    std::fs::write(local.join("staged.txt"), "local change\n").unwrap();
    run_git_ok(&local, &["add", "staged.txt"]);

    let (status, _, _) = check_local_status_no_fetch(tmp.join("repos").to_str().unwrap(), "myrepo");

    assert_eq!(status, LocalStatus::Staging);
    assert!(local_head_matches_upstream(
        tmp.join("repos").to_str().unwrap(),
        "myrepo"
    ));

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn local_head_matches_upstream_returns_false_after_remote_advances() {
    let (tmp, seed, local) = setup_remote_with_clone("different_head_modified");

    std::fs::write(local.join("local-only.txt"), "local change\n").unwrap();
    std::fs::write(seed.join("remote-only.txt"), "remote change\n").unwrap();
    run_git_ok(&seed, &["add", "remote-only.txt"]);
    run_git_ok(&seed, &["commit", "-m", "remote update"]);
    run_git_ok(&seed, &["push", "origin", "HEAD"]);
    run_git_ok(&local, &["fetch", "origin"]);

    let (status, _, _) = check_local_status_no_fetch(tmp.join("repos").to_str().unwrap(), "myrepo");

    assert_eq!(status, LocalStatus::Modified);
    assert!(!local_head_matches_upstream(
        tmp.join("repos").to_str().unwrap(),
        "myrepo"
    ));

    std::fs::remove_dir_all(&tmp).ok();
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

    let (status, has_local_git, files) =
        check_local_status_no_fetch(tmp.to_str().unwrap(), "myrepo");

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

    let (status_before, _, _) =
        check_local_status_no_fetch(tmp.join("repos").to_str().unwrap(), "myrepo");
    assert_eq!(status_before, LocalStatus::Modified);

    git_pull(tmp.join("repos").to_str().unwrap(), "myrepo").unwrap();

    assert!(local.join("local-only.txt").exists());
    assert!(local.join("remote-only.txt").exists());
    let (status_after, _, files_after) =
        check_local_status_no_fetch(tmp.join("repos").to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status_after, LocalStatus::Modified);
    assert!(files_after
        .iter()
        .any(|line| line.contains("local-only.txt")));
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

    let (status_before, _, _) =
        check_local_status_no_fetch(tmp.join("repos").to_str().unwrap(), "myrepo");
    assert_eq!(status_before, LocalStatus::Staging);

    git_pull(tmp.join("repos").to_str().unwrap(), "myrepo").unwrap();

    assert!(local.join("staged.txt").exists());
    assert!(local.join("remote-only.txt").exists());
    let (status_after, _, files_after) =
        check_local_status_no_fetch(tmp.join("repos").to_str().unwrap(), "myrepo");
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

    let (status_after, _, files_after) =
        check_local_status_no_fetch(tmp.join("repos").to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(status_after, LocalStatus::Conflict);
    assert!(files_after.iter().any(|line| line.starts_with("UU ")));
}

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
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
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
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
}

#[test]
fn check_workflows_empty_dir_returns_false() {
    let tmp = std::env::temp_dir().join(format!("wf_test_c_{}", std::process::id()));
    let repo = tmp.join("myrepo");
    std::fs::create_dir_all(&repo).unwrap();
    let result = check_workflows(tmp.to_str().unwrap(), "myrepo");
    std::fs::remove_dir_all(&tmp).ok();
    assert!(!result);
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
