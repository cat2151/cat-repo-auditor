use super::*;
use crate::main_helpers::BACKGROUND_CHECKS_COMPLETED_MSG;
use std::{fs, path::PathBuf, sync::Mutex};

static LOG_TEST_MUTEX: Mutex<()> = Mutex::new(());

struct TempLogDir {
    root: PathBuf,
}

impl TempLogDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "catrepo-main-fetch-tests-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("should create temp log dir for test");
        Self { root }
    }

    fn log_path(&self) -> PathBuf {
        Config::log_path_from_config_dir(&self.root)
    }
}

impl Drop for TempLogDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn drain_fetch_channel_persists_background_checks_completed_log() {
    let _guard = LOG_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = App::new(make_config());
    let temp_log_dir = TempLogDir::new();
    let log_path = temp_log_dir.log_path();
    fs::create_dir_all(
        log_path
            .parent()
            .expect("log path should have parent directory"),
    )
    .expect("should create log directory for test");

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::BackgroundChecksCompleted).unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel_for_log_path(&mut app, &mut fetch_rx, log_path.as_path());

    let persisted = fs::read_to_string(&log_path).unwrap();
    assert!(persisted.contains(BACKGROUND_CHECKS_COMPLETED_MSG));
    assert!(app
        .log_lines
        .last()
        .is_some_and(|line| line.contains(BACKGROUND_CHECKS_COMPLETED_MSG)));
}

#[test]
fn drain_fetch_channel_persists_log_messages() {
    let _guard = LOG_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = App::new(make_config());
    let temp_log_dir = TempLogDir::new();
    let log_path = temp_log_dir.log_path();
    fs::create_dir_all(
        log_path
            .parent()
            .expect("log path should have parent directory"),
    )
    .expect("should create log directory for test");

    let (tx, rx) = mpsc::channel();
    tx.send(FetchProgress::Log(String::from("pull owner/repo: ok")))
        .unwrap();
    drop(tx);

    let mut fetch_rx = Some(rx);
    drain_fetch_channel_for_log_path(&mut app, &mut fetch_rx, log_path.as_path());

    let persisted = fs::read_to_string(&log_path).unwrap();
    assert!(persisted.contains("pull owner/repo: ok"));
    assert!(app
        .log_lines
        .last()
        .is_some_and(|line| line.contains("pull owner/repo: ok")));
}
