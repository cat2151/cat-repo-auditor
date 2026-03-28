use super::*;

#[test]
fn build_commit_hash_is_not_empty() {
    assert!(!build_commit_hash().is_empty());
}

#[test]
fn update_available_when_hashes_differ() {
    assert!(is_update_available("aabbcc", "ddeeff"));
}

#[test]
fn no_update_when_hashes_equal() {
    assert!(!is_update_available("aabbcc", "aabbcc"));
}

#[test]
fn no_update_when_build_hash_unknown() {
    assert!(!is_update_available("unknown", "ddeeff"));
}

#[test]
fn no_update_when_build_hash_empty() {
    assert!(!is_update_available("", "ddeeff"));
}

#[test]
fn no_update_when_remote_hash_empty() {
    assert!(!is_update_available("aabbcc", ""));
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
