use chrono::Local;
use std::{
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Write},
    sync::mpsc,
};

use crate::{
    app::App,
    config::Config,
    github::{fetch_repos_with_progress, FetchProgress},
    history::History,
};

pub(crate) fn start_fetch(config: Config, history: History) -> mpsc::Receiver<FetchProgress> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || { fetch_repos_with_progress(config, history, tx); });
    rx
}

pub(crate) fn read_log_lines() -> Vec<String> {
    let path = Config::log_path();
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    BufReader::new(f)
        .lines()
        .map_while(std::result::Result::ok)
        .collect()
}

fn append_log_line(line: &str) -> io::Result<()> {
    let path = Config::log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{line}")?;
    f.flush()
}

pub(crate) fn make_x_log_line(repo_full_name: &str, msg: &str) -> String {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    format!("[{now}] x {repo_full_name} {msg}")
}

pub(crate) fn persist_log_line(app: &mut App, line: String) {
    if let Err(e) = append_log_line(&line) {
        app.transient_msg = Some(format!("log write failed: {e}"));
    } else {
        app.append_log_line(line);
    }
}
