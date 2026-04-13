use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn sanitize_path_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        String::from("unnamed")
    } else {
        sanitized
    }
}

fn unique_history_test_path(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let current_thread = std::thread::current();
    let thread_name = current_thread.name().unwrap_or("unnamed");
    let safe_thread_name = sanitize_path_component(thread_name);
    std::env::temp_dir().join(format!(
        "{prefix}_{}_{}_{}.json",
        std::process::id(),
        safe_thread_name,
        nanos
    ))
}

#[test]
fn sanitize_path_component_replaces_windows_invalid_characters() {
    assert_eq!(
        sanitize_path_component("history::tests/save_and_load_roundtrip"),
        "history__tests_save_and_load_roundtrip"
    );
}

#[test]
fn save_and_load_roundtrip() {
    let tmp = unique_history_test_path("cat_repo_auditor_history_test");
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
    let path = unique_history_test_path("cat_repo_auditor_no_such_file");
    // Ensure the file does not exist before testing
    std::fs::remove_file(&path).ok();
    let result = History::load(path.to_str().unwrap());
    assert!(result.is_err());
}

#[test]
fn save_and_load_with_rate_limit() {
    use crate::github::RateLimit;

    let tmp = unique_history_test_path("cat_repo_auditor_rl_test");
    let path_str = tmp.to_str().unwrap();

    let history = History {
        rate_limit: Some(RateLimit {
            remaining: 4000,
            limit: 5000,
            reset_at: String::from("2024-01-01T01:00:00Z"),
        }),
        ..History::default()
    };

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

#[test]
fn update_creates_file_when_missing() {
    let tmp = unique_history_test_path("cat_repo_auditor_history_update_missing");
    std::fs::remove_file(&tmp).ok();
    let path_str = tmp.to_str().unwrap();

    History::update(path_str, |history| {
        history
            .etags
            .insert(String::from("owner"), String::from("etag456"));
    })
    .unwrap();

    let loaded = History::load(path_str).unwrap();
    assert_eq!(loaded.etags.get("owner").unwrap(), "etag456");
    std::fs::remove_file(&tmp).ok();
}
