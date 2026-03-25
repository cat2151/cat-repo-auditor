use super::*;

#[test]
fn save_and_load_roundtrip() {
    let tmp = std::env::temp_dir().join(format!(
        "cat_repo_auditor_history_test_{}.json",
        std::process::id()
    ));
    let path_str = tmp.to_str().unwrap();

    let mut history = History::default();
    history
        .etags
        .insert(String::from("owner"), String::from("etag123"));

    history.save(path_str).unwrap();
    let loaded = History::load(path_str).unwrap();

    assert_eq!(loaded.etags.get("owner").unwrap(), "etag123");
    assert!(loaded.repos.is_empty());
    assert!(loaded.rate_limit.is_none());
    std::fs::remove_file(&tmp).ok();
}

#[test]
fn load_nonexistent_file_returns_error() {
    let path = std::env::temp_dir().join(format!(
        "cat_repo_auditor_no_such_file_{}.json",
        std::process::id()
    ));
    // Ensure the file does not exist before testing
    std::fs::remove_file(&path).ok();
    let result = History::load(path.to_str().unwrap());
    assert!(result.is_err());
}

#[test]
fn save_and_load_with_rate_limit() {
    use crate::github::RateLimit;

    let tmp = std::env::temp_dir().join(format!(
        "cat_repo_auditor_rl_test_{}.json",
        std::process::id()
    ));
    let path_str = tmp.to_str().unwrap();

    let mut history = History::default();
    history.rate_limit = Some(RateLimit {
        remaining: 4000,
        limit: 5000,
        reset_at: String::from("2024-01-01T01:00:00Z"),
    });

    history.save(path_str).unwrap();
    let loaded = History::load(path_str).unwrap();

    let rl = loaded.rate_limit.expect("rate_limit should be present");
    assert_eq!(rl.remaining, 4000);
    assert_eq!(rl.limit, 5000);
    assert_eq!(rl.reset_at, "2024-01-01T01:00:00Z");
    std::fs::remove_file(&tmp).ok();
}

#[test]
fn default_history_has_empty_fields() {
    let history = History::default();
    assert!(history.etags.is_empty());
    assert!(history.repos.is_empty());
    assert!(history.rate_limit.is_none());
}
