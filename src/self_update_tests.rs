use super::*;

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
fn bat_content_contains_install_command() {
    let bat = update_bat_content();
    assert!(bat.contains("cargo install --force --git"));
    assert!(bat.contains("cat-repo-auditor"));
}

#[test]
fn bat_content_has_delay() {
    let bat = update_bat_content();
    assert!(bat.contains("timeout"));
}

#[test]
fn bat_content_self_deletes() {
    let bat = update_bat_content();
    assert!(bat.contains("del"));
    assert!(bat.contains("%~f0"));
}
