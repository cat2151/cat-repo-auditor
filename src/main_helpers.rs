use chrono::Local;
use std::{
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    sync::mpsc,
    time::SystemTime,
};

use crate::{
    app::App,
    config::Config,
    github::{fetch_repos_with_progress, FetchProgress},
    history::History,
};

pub(crate) fn start_fetch(config: Config, history: History) -> mpsc::Receiver<FetchProgress> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        fetch_repos_with_progress(config, history, tx);
    });
    rx
}

pub(crate) fn read_log_lines_from_path(path: &Path) -> Vec<String> {
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    BufReader::new(f)
        .lines()
        .map_while(std::result::Result::ok)
        .collect()
}

pub(crate) fn log_last_modified_for_path(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
}

pub(crate) fn refresh_log_lines_if_changed_for_path(app: &mut App, path: &Path) {
    let last_modified = log_last_modified_for_path(path);
    if app.log_last_modified != last_modified {
        app.set_log_lines(read_log_lines_from_path(path));
        app.log_last_modified = last_modified;
    }
}

pub(crate) fn refresh_log_lines_if_changed(app: &mut App) {
    refresh_log_lines_if_changed_for_path(app, &Config::log_path());
}

fn append_log_line_for_path(path: &Path, line: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{line}")?;
    f.flush()
}

pub(crate) fn make_x_log_line(repo_full_name: &str, msg: &str) -> String {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    format!("[{now}] x {repo_full_name} {msg}")
}

pub(crate) fn make_log_line(msg: &str) -> String {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    format!("[{now}] {msg}")
}

pub(crate) const BACKGROUND_CHECKS_COMPLETED_LOG_MSG: &str = "background checks completed";
pub(crate) const STARTUP_LOG_SEPARATOR: &str = "---";
pub(crate) const STARTUP_LOG_MSG: &str = "catrepo started";

pub(crate) fn make_startup_log_line() -> String {
    make_log_line(STARTUP_LOG_MSG)
}

pub(crate) fn persist_log_line_for_path(app: &mut App, path: &Path, line: String) {
    if let Err(e) = append_log_line_for_path(path, &line) {
        app.transient_msg = Some(format!("log write failed: {e}"));
    } else {
        app.append_log_line(line);
        app.log_last_modified = log_last_modified_for_path(path);
    }
}

pub(crate) fn persist_log_line(app: &mut App, line: String) {
    persist_log_line_for_path(app, &Config::log_path(), line);
}
