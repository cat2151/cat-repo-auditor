use super::helpers::*;
use super::*;
use std::process::Command as Cmd;

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
fn local_head_matches_upstream_logs_start_hashes_and_result() {
    let (tmp, _seed, _local) = setup_remote_with_clone("same_head_logged");
    let mut logs = Vec::new();

    let matches = local_head_matches_upstream_with_logger(
        tmp.join("repos").to_str().unwrap(),
        "myrepo",
        |msg| logs.push(msg.to_string()),
    );

    std::fs::remove_dir_all(&tmp).ok();

    assert!(matches);
    assert!(logs.iter().any(|msg| {
        msg.contains("local repo check:")
            && msg.contains("リポジトリ=myrepo")
            && msg.contains("開始: ローカルとリモートのコミットハッシュ比較を開始します")
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("ローカルのコミットハッシュ取得を開始します")
            && msg.contains("git -C")
            && msg.contains("rev-parse HEAD")
    }));
    assert!(logs
        .iter()
        .any(|msg| msg.contains("ローカルのコミットハッシュを取得しました:")));
    assert!(logs.iter().any(|msg| {
        msg.contains("リモートから取得したコミットハッシュの取得を開始します")
            && msg.contains("rev-parse @{u}")
    }));
    assert!(logs
        .iter()
        .any(|msg| msg.contains("リモートから取得したコミットハッシュを取得しました:")));
    assert!(logs.iter().any(|msg| {
        msg.contains("ローカルとリモートのコミットハッシュ比較結果=一致")
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("終了: ローカル repo check を完了しました") && msg.contains("比較結果=一致")
    }));
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
