use super::*;
use crate::config::Config;
use crate::github::RateLimit;

fn make_config() -> Config {
    Config {
        owner: String::from("owner"),
        local_base_dir: String::from("."),
        app_run_dir: None,
        auto_pull: false,
    }
}

#[test]
fn drain_fetch_channel_applies_done_ok_and_disconnect_cleanup() {
    let mut app = App::new(make_config());
    app.bg_tasks.push(("chk", 1, 1));
    app.checking_repo = String::from("repo");

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::Done(Ok((
        vec![],
        RateLimit {
            remaining: 9,
            limit: 60,
            reset_at: String::from("2026-01-01T00:00:00Z"),
        },
    ))))
    .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel(&mut app, &mut fetch_rx);

    assert_eq!(app.rate_limit.as_ref().map(|r| r.remaining), Some(9));
    assert!(!app.loading);
    assert_eq!(app.status_msg, READY_MSG);
    assert!(fetch_rx.is_none());
    assert!(app.bg_tasks.is_empty());
    assert!(app.checking_repo.is_empty());
}
