use super::*;

#[test]
fn build_commit_hash_is_not_empty() {
    assert!(!build_commit_hash().is_empty());
}

#[test]
fn install_command_contains_install_git_url() {
    let cmd = install_cmd();
    assert!(cmd.contains("cargo install --force --git"));
    assert!(cmd.contains("cat-repo-auditor"));
}

#[test]
fn install_command_targets_repository_url() {
    assert_eq!(
        install_cmd(),
        "cargo install --force --git https://github.com/cat2151/cat-repo-auditor"
    );
}
