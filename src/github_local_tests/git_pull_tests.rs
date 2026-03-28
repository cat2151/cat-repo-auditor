use super::helpers::*;
use super::*;

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
