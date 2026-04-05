use std::sync::mpsc;

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::FetchProgress,
    main_helpers::{make_log_line, persist_log_line_for_path, BACKGROUND_CHECKS_COMPLETED_MSG},
};
use std::path::Path;

pub(crate) fn drain_fetch_channel(
    app: &mut App,
    fetch_rx: &mut Option<mpsc::Receiver<FetchProgress>>,
) {
    drain_fetch_channel_for_log_path(app, fetch_rx, &Config::log_path());
}

pub(crate) fn drain_fetch_channel_for_log_path(
    app: &mut App,
    fetch_rx: &mut Option<mpsc::Receiver<FetchProgress>>,
    log_path: &Path,
) {
    while let Some(result) = fetch_rx.as_ref().map(mpsc::Receiver::try_recv) {
        match result {
            Ok(FetchProgress::Status(_msg)) => {
                // status_msg stays as operation help.
            }
            Ok(FetchProgress::Log(msg)) => {
                persist_log_line_for_path(app, log_path, make_log_line(&msg))
            }
            Ok(FetchProgress::BackgroundChecksCompleted) => persist_log_line_for_path(
                app,
                log_path,
                make_log_line(BACKGROUND_CHECKS_COMPLETED_MSG),
            ),
            Ok(FetchProgress::PhaseProgress { tag, cur, total }) => {
                if cur == 0 && total == 0 {
                    // Clear signal
                    app.bg_tasks.retain(|t| t.0 != tag);
                } else if let Some(entry) = app.bg_tasks.iter_mut().find(|t| t.0 == tag) {
                    // Upsert
                    entry.1 = cur;
                    entry.2 = total;
                } else {
                    app.bg_tasks.push((tag, cur, total));
                }
            }
            Ok(FetchProgress::CheckingRepo(name)) => {
                if name.is_empty() {
                    app.checking_repos.clear();
                } else if !app.checking_repos.iter().any(|checking| checking == &name) {
                    app.checking_repos.push(name);
                }
            }
            Ok(FetchProgress::ExistenceUpdate {
                name,
                readme_ja,
                readme_ja_cat,
                readme_ja_badge,
                readme_ja_badge_cat,
                pages,
                pages_cat,
                deepwiki,
                deepwiki_cat,
                cargo_install,
                cargo_cat,
                cargo_remote_hash,
                cargo_remote_hash_cat,
                cargo_installed_hash,
                wf_workflows,
                wf_cat,
            }) => {
                if let Some(r) = app.repos.iter_mut().find(|r| r.name == name) {
                    r.readme_ja = readme_ja;
                    r.readme_ja_checked_at = readme_ja_cat;
                    r.readme_ja_badge = readme_ja_badge;
                    r.readme_ja_badge_checked_at = readme_ja_badge_cat;
                    r.pages = pages;
                    r.pages_checked_at = pages_cat;
                    r.deepwiki = deepwiki;
                    r.deepwiki_checked_at = deepwiki_cat;
                    r.cargo_install = cargo_install;
                    r.cargo_checked_at = cargo_cat;
                    r.cargo_remote_hash = cargo_remote_hash;
                    r.cargo_remote_hash_checked_at = cargo_remote_hash_cat;
                    r.cargo_installed_hash = cargo_installed_hash;
                    r.wf_workflows = wf_workflows;
                    r.wf_checked_at = wf_cat;
                }
                app.checking_repos.retain(|checking| checking != &name);
            }
            Ok(FetchProgress::Done(Ok((repos, rl)))) => {
                app.repos = repos;
                app.rate_limit = Some(rl);
                app.rebuild_rows();
                app.loading = false;
                app.status_msg = String::from(READY_MSG);
                // Do NOT set fetch_rx = None here:
                // phase 3 (CheckingRepo / ExistenceUpdate) messages come after Done.
                // Keep draining until Disconnected.
            }
            Ok(FetchProgress::Done(Err(e))) => {
                app.loading = false;
                app.status_msg = format!("Error: {e}");
                *fetch_rx = None;
                break;
            }
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => {
                *fetch_rx = None;
                app.bg_tasks.clear();
                app.checking_repos.clear();
                break;
            }
        }
    }
}

#[cfg(test)]
#[path = "main_fetch_tests.rs"]
mod tests;
