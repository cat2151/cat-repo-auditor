use super::support::{make_config, make_repo};
use super::*;
use std::time::{Duration, SystemTime};

#[test]
fn start_cargo_hash_polling_schedules_first_check_after_one_minute() {
    let mut app = App::new(make_config());
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(123);

    app.start_cargo_hash_polling_at("repo", now);

    assert_eq!(app.active_cargo_hash_poll_count(), 1);
    assert!(app.due_cargo_hash_polls_at(now).is_empty());
    assert_eq!(
        app.due_cargo_hash_polls_at(now + CARGO_HASH_POLL_INTERVAL),
        vec![String::from("repo")]
    );
}

#[test]
fn finish_cargo_hash_poll_attempt_reschedules_until_timeout() {
    let mut app = App::new(make_config());
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(500);

    app.start_cargo_hash_polling_at("repo", now);
    app.mark_cargo_hash_poll_in_flight("repo");

    assert!(!app.finish_cargo_hash_poll_attempt_at("repo", now + Duration::from_secs(90)));
    assert!(app
        .due_cargo_hash_polls_at(now + Duration::from_secs(149))
        .is_empty());
    assert_eq!(
        app.due_cargo_hash_polls_at(now + Duration::from_secs(150)),
        vec![String::from("repo")]
    );
}

#[test]
fn expire_cargo_hash_polls_removes_timed_out_repo() {
    let mut app = App::new(make_config());
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);

    app.start_cargo_hash_polling_at("repo", now);

    let expired = app.expire_cargo_hash_polls_at(now + CARGO_HASH_POLL_TIMEOUT);

    assert_eq!(expired, vec![String::from("repo")]);
    assert_eq!(app.active_cargo_hash_poll_count(), 0);
}

#[test]
fn expire_cargo_hash_polls_removes_timed_out_repo_even_if_in_flight() {
    let mut app = App::new(make_config());
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000);

    app.start_cargo_hash_polling_at("repo", now);
    app.mark_cargo_hash_poll_in_flight("repo");

    let expired = app.expire_cargo_hash_polls_at(now + CARGO_HASH_POLL_TIMEOUT);

    assert_eq!(expired, vec![String::from("repo")]);
    assert_eq!(app.active_cargo_hash_poll_count(), 0);
}

#[test]
fn take_expired_cargo_hash_polls_preserves_auto_update_source() {
    let mut app = App::new(make_config());
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(3_000);

    app.start_auto_update_cargo_hash_polling("repo");
    app.cargo_hash_polls[0].started_at = now - CARGO_HASH_POLL_TIMEOUT;

    let expired = app.take_expired_cargo_hash_polls_at(now);

    assert_eq!(
        expired,
        vec![ExpiredCargoHashPoll {
            repo_name: String::from("repo"),
            after_auto_update: true,
        }]
    );
    assert_eq!(app.active_cargo_hash_poll_count(), 0);
}

#[test]
fn stop_cargo_hash_polling_removes_existing_entry() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("repo")];

    app.start_cargo_hash_polling_at("repo", SystemTime::UNIX_EPOCH);
    app.stop_cargo_hash_polling("repo");

    assert_eq!(app.active_cargo_hash_poll_count(), 0);
}
