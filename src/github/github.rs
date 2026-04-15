use crate::{
    config::Config,
    github_fetch::do_fetch,
    github_local::{
        check_local_status_no_fetch, git_pull, local_head_hash_no_fetch,
        local_head_matches_upstream,
    },
    history::History,
    self_update,
};

#[path = "github_cargo_worker.rs"]
mod cargo_worker;
#[path = "github_phase3.rs"]
mod phase3;

#[path = "github_types.rs"]
mod types;

use cargo_worker::{apply_cargo_result_to_history, spawn_background_cargo_checks};
use phase3::{
    apply_phase3_result, build_phase3_tasks, collect_local_heads, phase3_worker_count,
    run_phase3_repo_task, spawn_background_local_checks,
};

pub use types::{
    AutoUpdateLaunchRequest, FetchProgress, IssueOrPr, LocalStatus, RateLimit, RepoInfo,
};

// ──────────────────────────────────────────────
// Public types
// ──────────────────────────────────────────────

type PullTarget = (String, String);

fn split_startup_and_post_fetch_cargo_repos(
    cached_repos: &[RepoInfo],
    fetched_repos: &[RepoInfo],
) -> (Vec<RepoInfo>, Vec<RepoInfo>) {
    if cached_repos.is_empty() {
        return (vec![], fetched_repos.to_vec());
    }

    let cached_repo_names: std::collections::HashSet<&str> =
        cached_repos.iter().map(|repo| repo.name.as_str()).collect();
    let post_fetch_repos = fetched_repos
        .iter()
        .filter(|repo| !cached_repo_names.contains(repo.name.as_str()))
        .cloned()
        .collect();

    (cached_repos.to_vec(), post_fetch_repos)
}

fn refresh_repos_after_auto_pull_with<CheckLocalStatus, LocalHeadHash>(
    repos: &mut [RepoInfo],
    base_dir: &str,
    refreshed_repo_names: &[String],
    check_local_status: CheckLocalStatus,
    local_head_hash: LocalHeadHash,
) where
    CheckLocalStatus: Fn(&str, &str) -> (LocalStatus, bool, Vec<String>),
    LocalHeadHash: Fn(&str, &str) -> String,
{
    let refreshed_repo_names: std::collections::HashSet<&str> =
        refreshed_repo_names.iter().map(String::as_str).collect();

    for repo in repos
        .iter_mut()
        .filter(|repo| refreshed_repo_names.contains(repo.name.as_str()))
    {
        let (local_status, has_local_git, staging_files) = check_local_status(base_dir, &repo.name);
        repo.local_status = local_status;
        repo.has_local_git = has_local_git;
        repo.staging_files = staging_files;
        repo.local_head_hash = local_head_hash(base_dir, &repo.name);
    }
}

fn refresh_repos_after_auto_pull(
    repos: &mut [RepoInfo],
    base_dir: &str,
    refreshed_repo_names: &[String],
) {
    refresh_repos_after_auto_pull_with(
        repos,
        base_dir,
        refreshed_repo_names,
        check_local_status_no_fetch,
        local_head_hash_no_fetch,
    );
}

fn should_auto_pull_status(local_status: &LocalStatus, head_matches_upstream: bool) -> bool {
    match local_status {
        LocalStatus::Pullable => true,
        LocalStatus::Modified | LocalStatus::Staging => !head_matches_upstream,
        _ => false,
    }
}

fn should_auto_pull_repo(base_dir: &str, repo: &RepoInfo) -> bool {
    let head_matches_upstream = matches!(
        repo.local_status,
        LocalStatus::Modified | LocalStatus::Staging
    ) && local_head_matches_upstream(base_dir, &repo.name);
    should_auto_pull_status(&repo.local_status, head_matches_upstream)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AutoUpdateAfterRecheck {
    NotOldBeforeRecheck,
    RecheckFailed,
    StillOld {
        installed_hash: String,
        remote_hash: String,
    },
    UpdatedDuringRecheck {
        installed_hash: String,
        remote_hash: String,
    },
}

fn inspect_auto_update_after_recheck<Recheck>(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_install: Option<bool>,
    recheck: Recheck,
) -> AutoUpdateAfterRecheck
where
    Recheck: FnOnce(&str, &str, &str) -> Option<(bool, String, String, String)>,
{
    if cargo_install != Some(false) {
        return AutoUpdateAfterRecheck::NotOldBeforeRecheck;
    }

    match recheck(owner, repo_name, base_dir) {
        Some((false, installed_hash, _local_hash, remote_hash)) => {
            AutoUpdateAfterRecheck::StillOld {
                installed_hash,
                remote_hash,
            }
        }
        Some((true, installed_hash, _local_hash, remote_hash)) => {
            AutoUpdateAfterRecheck::UpdatedDuringRecheck {
                installed_hash,
                remote_hash,
            }
        }
        None => AutoUpdateAfterRecheck::RecheckFailed,
    }
}

pub(super) fn should_skip_auto_update_for_repo(owner: &str, repo_name: &str) -> bool {
    owner.eq_ignore_ascii_case(self_update::REPO_OWNER)
        && repo_name.eq_ignore_ascii_case(self_update::REPO_NAME)
}

#[cfg(test)]
fn should_spawn_auto_update_after_recheck<Recheck>(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_install: Option<bool>,
    recheck: Recheck,
) -> bool
where
    Recheck: FnOnce(&str, &str, &str) -> Option<(bool, String, String, String)>,
{
    !should_skip_auto_update_for_repo(owner, repo_name)
        && matches!(
            inspect_auto_update_after_recheck(owner, repo_name, base_dir, cargo_install, recheck),
            AutoUpdateAfterRecheck::StillOld { .. }
        )
}

fn compact_log_detail(detail: &str) -> String {
    detail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn format_pull_log(repo_full_name: &str, pull_result: &anyhow::Result<String>) -> String {
    match pull_result {
        Ok(output) => {
            let detail = compact_log_detail(output);
            if detail.is_empty() {
                format!("pull {repo_full_name}: ok")
            } else {
                format!("pull {repo_full_name}: {detail}")
            }
        }
        Err(err) => format!(
            "pull {repo_full_name} failed: {}",
            compact_log_detail(&format!("{err:#}"))
        ),
    }
}

// ──────────────────────────────────────────────
// Fetch orchestration
// ──────────────────────────────────────────────

pub fn fetch_repos_with_progress(
    config: Config,
    mut history: History,
    tx: std::sync::mpsc::Sender<FetchProgress>,
) {
    let startup_local_repos = history.repos.clone();
    let (startup_cargo_repos, _) = split_startup_and_post_fetch_cargo_repos(&history.repos, &[]);
    let owner = config.owner.clone();
    let auto_update_run_dir = if config.auto_update {
        Some(config.resolved_app_run_dir())
    } else {
        None
    };
    let startup_cargo_handle = if startup_cargo_repos.is_empty() {
        None
    } else {
        let _ = tx.send(FetchProgress::BeginCargoRefresh(
            startup_cargo_repos
                .iter()
                .map(|repo| repo.name.clone())
                .collect(),
        ));
        let local_heads = collect_local_heads(&startup_cargo_repos, &config.local_base_dir);
        Some(spawn_background_cargo_checks(
            &startup_cargo_repos,
            &local_heads,
            &owner,
            &config.local_base_dir,
            auto_update_run_dir.as_deref(),
            &tx,
        ))
    };
    let startup_local_handle = if startup_local_repos.is_empty() {
        None
    } else {
        let _ = tx.send(FetchProgress::BeginLocalRefresh(
            startup_local_repos
                .iter()
                .map(|repo| repo.name.clone())
                .collect(),
        ));
        Some(spawn_background_local_checks(
            &startup_local_repos,
            &config.local_base_dir,
            &tx,
        ))
    };

    if let Some(cargo_handle) = startup_cargo_handle {
        if let Ok(cargo_results) = cargo_handle.join() {
            for result in &cargo_results {
                apply_cargo_result_to_history(&mut history, result);
            }
        }
    }
    if let Some(local_handle) = startup_local_handle {
        if let Ok(local_results) = local_handle.join() {
            for result in &local_results {
                if let Some(r) = history.repos.iter_mut().find(|r| r.name == result.name) {
                    apply_phase3_result(r, result);
                }
            }
        }
    }

    // Phase 1: fetch repo list
    let result = do_fetch(&config, &mut history, &tx);
    match result {
        Err(e) => {
            let _ = tx.send(FetchProgress::Done(Err(e)));
        }
        Ok((mut repos, rl)) => {
            let _ = tx.send(FetchProgress::Done(Ok((repos.clone(), rl))));

            let (_, post_fetch_cargo_repos) =
                split_startup_and_post_fetch_cargo_repos(&startup_cargo_repos, &repos);
            let post_fetch_cargo_handle = if post_fetch_cargo_repos.is_empty() {
                None
            } else {
                let _ = tx.send(FetchProgress::BeginCargoRefresh(
                    post_fetch_cargo_repos
                        .iter()
                        .map(|repo| repo.name.clone())
                        .collect(),
                ));
                let local_heads =
                    collect_local_heads(&post_fetch_cargo_repos, &config.local_base_dir);
                Some(spawn_background_cargo_checks(
                    &post_fetch_cargo_repos,
                    &local_heads,
                    &owner,
                    &config.local_base_dir,
                    auto_update_run_dir.as_deref(),
                    &tx,
                ))
            };

            // Phase 2: auto-pull repos that can be safely fast-forwarded.
            // Dirty repos are handled by stashing before pull and restoring after.
            let pullable: Vec<PullTarget> = if config.auto_pull {
                repos
                    .iter()
                    .filter(|r| should_auto_pull_repo(&config.local_base_dir, r))
                    .map(|r| (r.name.clone(), r.full_name.clone()))
                    .collect()
            } else {
                vec![]
            };
            if !pullable.is_empty() {
                let total = pullable.len();
                let mut refreshed_repo_names = Vec::with_capacity(total);
                for (i, (name, repo_full_name)) in pullable.iter().enumerate() {
                    refreshed_repo_names.push(name.clone());
                    let _ = tx.send(FetchProgress::PhaseProgress {
                        tag: "pull",
                        cur: i + 1,
                        total,
                    });
                    let pull_result = git_pull(&config.local_base_dir, name);
                    let _ = tx.send(FetchProgress::Log(format_pull_log(
                        repo_full_name,
                        &pull_result,
                    )));
                }
                refresh_repos_after_auto_pull(
                    &mut repos,
                    &config.local_base_dir,
                    &refreshed_repo_names,
                );
            }

            // Phase 3:
            // - README / Pages / DeepWiki / workflows は各 checked_at が古いときだけ再確認する。
            // - local clean は startup background で先行実行し、ここでは残りの existence checks を行う。
            // - cargo install 状態の確認は auto-pull / existence check とは独立して先行実行する。
            let phase3_tasks = build_phase3_tasks(&repos);

            if !phase3_tasks.is_empty() {
                let total_check = phase3_tasks.len();
                let worker_count = phase3_worker_count(total_check);
                for task in &phase3_tasks {
                    let _ = tx.send(FetchProgress::CheckingRepo(task.repo.name.clone()));
                }
                let (phase3_result_tx, phase3_result_rx) = std::sync::mpsc::channel();
                let work_queue = std::sync::Arc::new(std::sync::Mutex::new(
                    phase3_tasks
                        .into_iter()
                        .collect::<std::collections::VecDeque<_>>(),
                ));

                std::thread::scope(|scope| {
                    for _ in 0..worker_count {
                        let work_queue = std::sync::Arc::clone(&work_queue);
                        let phase3_result_tx = phase3_result_tx.clone();
                        let owner = owner.clone();
                        let base_dir = config.local_base_dir.clone();
                        scope.spawn(move || {
                            while let Some(task) = {
                                let mut work_queue =
                                    work_queue.lock().unwrap_or_else(|e| e.into_inner());
                                work_queue.pop_front()
                            } {
                                let result = run_phase3_repo_task(task, &owner, &base_dir);
                                let _ = phase3_result_tx.send(result);
                            }
                        });
                    }
                    drop(phase3_result_tx);

                    for (completed, result) in phase3_result_rx.into_iter().enumerate() {
                        let _ = tx.send(FetchProgress::PhaseProgress {
                            tag: "chk",
                            cur: completed + 1,
                            total: total_check,
                        });

                        if let Some(r) = history.repos.iter_mut().find(|r| r.name == result.name) {
                            apply_phase3_result(r, &result);
                        }

                        let _ = tx.send(FetchProgress::ExistenceUpdate {
                            name: result.name.clone(),
                            local_status: result.local_status,
                            has_local_git: result.has_local_git,
                            staging_files: result.staging_files.clone(),
                            local_head_hash: result.local_head_hash.clone(),
                            readme_ja: result.readme_ja,
                            readme_ja_cat: result.readme_ja_cat.clone(),
                            readme_ja_badge: result.readme_ja_badge,
                            readme_ja_badge_cat: result.readme_ja_badge_cat.clone(),
                            pages: result.pages,
                            pages_cat: result.pages_cat.clone(),
                            deepwiki: result.deepwiki,
                            deepwiki_cat: result.deepwiki_cat.clone(),
                            wf_workflows: result.wf_workflows,
                            wf_cat: result.wf_cat.clone(),
                        });
                    }
                });
            }

            for cargo_handle in post_fetch_cargo_handle.into_iter() {
                if let Ok(cargo_results) = cargo_handle.join() {
                    for result in &cargo_results {
                        apply_cargo_result_to_history(&mut history, result);
                    }
                }
            }

            let _ = tx.send(FetchProgress::BackgroundChecksCompleted);
            // Clear progress indicators
            let _ = tx.send(FetchProgress::CheckingRepo(String::new()));
            let _ = tx.send(FetchProgress::PhaseProgress {
                tag: "chk",
                cur: 0,
                total: 0,
            });
            history
                .save(&crate::config::Config::history_path().to_string_lossy())
                .ok();
        }
    }
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
