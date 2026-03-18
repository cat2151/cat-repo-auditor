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
