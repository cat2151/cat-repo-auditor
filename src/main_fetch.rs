use std::sync::mpsc;

use crate::{
    app::{App, READY_MSG},
    config::Config,
    github::FetchProgress,
    main_helpers::{make_log_line, persist_log_line_for_path, BACKGROUND_CHECKS_COMPLETED_MSG},
};
use std::path::Path;

fn apply_cargo_update(
    repo: &mut crate::github::RepoInfo,
    cargo_install: Option<bool>,
    cargo_cat: String,
    cargo_remote_hash: String,
    cargo_remote_hash_cat: String,
    cargo_installed_hash: String,
) {
    repo.cargo_install = cargo_install;
    repo.cargo_checked_at = cargo_cat;
    repo.cargo_remote_hash = cargo_remote_hash;
    repo.cargo_remote_hash_checked_at = cargo_remote_hash_cat;
    repo.cargo_installed_hash = cargo_installed_hash;
}

fn has_live_cargo_state(repo: &crate::github::RepoInfo) -> bool {
    repo.cargo_install.is_some()
        || !repo.cargo_checked_at.is_empty()
        || !repo.cargo_remote_hash.is_empty()
        || !repo.cargo_remote_hash_checked_at.is_empty()
        || !repo.cargo_installed_hash.is_empty()
}

/// Merge cargo fields that were updated live after the previous `Done`.
///
/// `fetch_repos_with_progress()` can now send an initial `Done` after phase 1 and a second `Done`
/// after auto-pull refresh. If cargo checks finished in between those two snapshots, the incoming
/// refreshed repos would otherwise overwrite newer cargo state with older history-backed values.
/// This merge preserves only the live cargo fields for repos that already have such state.
fn merge_live_repo_state(
    existing_repos: &[crate::github::RepoInfo],
    incoming_repos: &mut [crate::github::RepoInfo],
) {
    for incoming in incoming_repos {
        if let Some(existing) = existing_repos.iter().find(|repo| repo.name == incoming.name) {
            if has_live_cargo_state(existing) {
                incoming.cargo_install = existing.cargo_install;
                incoming.cargo_checked_at = existing.cargo_checked_at.clone();
                incoming.cargo_remote_hash = existing.cargo_remote_hash.clone();
                incoming.cargo_remote_hash_checked_at =
                    existing.cargo_remote_hash_checked_at.clone();
                incoming.cargo_installed_hash = existing.cargo_installed_hash.clone();
            }
        }
    }
}

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
                } else {
                    app.checking_repos.insert(name);
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
                    r.wf_workflows = wf_workflows;
                    r.wf_checked_at = wf_cat;
                }
                app.checking_repos.remove(&name);
            }
            Ok(FetchProgress::CargoUpdate {
                name,
                cargo_install,
                cargo_cat,
                cargo_remote_hash,
                cargo_remote_hash_cat,
                cargo_installed_hash,
            }) => {
                if let Some(r) = app.repos.iter_mut().find(|r| r.name == name) {
                    apply_cargo_update(
                        r,
                        cargo_install,
                        cargo_cat,
                        cargo_remote_hash,
                        cargo_remote_hash_cat,
                        cargo_installed_hash,
                    );
                }
            }
            Ok(FetchProgress::Done(Ok((repos, rl)))) => {
                let mut repos = repos;
                merge_live_repo_state(&app.repos, &mut repos);
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
