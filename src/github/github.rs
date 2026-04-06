use crate::{
    config::Config,
    github_fetch::do_fetch,
    github_local::{
        check_deepwiki_exists, check_file_exists, check_pages_exists, check_readme_ja_badge,
        check_workflows, git_pull, local_head_matches_upstream,
    },
    history::History,
};

#[path = "github_cargo_worker.rs"]
mod cargo_worker;

#[path = "github_types.rs"]
mod types;

use cargo_worker::{apply_cargo_result_to_history, spawn_background_cargo_checks};

pub use types::{FetchProgress, IssueOrPr, LocalStatus, RateLimit, RepoInfo};

// ──────────────────────────────────────────────
// Public types
// ──────────────────────────────────────────────

type PullTarget = (String, String);

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
    cargo_install == Some(false)
        && matches!(recheck(owner, repo_name, base_dir), Some((false, _, _, _)))
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

#[derive(Clone)]
struct Phase3RepoTask {
    repo: RepoInfo,
    local_head: String,
}

struct Phase3RepoResult {
    name: String,
    readme_ja: Option<bool>,
    readme_ja_cat: String,
    readme_ja_badge: Option<bool>,
    readme_ja_badge_cat: String,
    pages: Option<bool>,
    pages_cat: String,
    deepwiki: Option<bool>,
    deepwiki_cat: String,
    wf_workflows: Option<bool>,
    wf_cat: String,
}

fn build_phase3_tasks(
    repos: &[RepoInfo],
    local_heads: &std::collections::HashMap<String, String>,
) -> Vec<Phase3RepoTask> {
    repos
        .iter()
        .map(|repo| Phase3RepoTask {
            repo: repo.clone(),
            local_head: local_heads.get(&repo.name).cloned().unwrap_or_default(),
        })
        .collect()
}

fn phase3_worker_count(total_check: usize) -> usize {
    debug_assert!(total_check > 0);
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4)
        .min(total_check)
}

fn collect_local_heads(
    repos: &[RepoInfo],
    local_base_dir: &str,
) -> std::collections::HashMap<String, String> {
    repos
        .iter()
        .filter(|r| r.has_local_git)
        .filter_map(|r| {
            let path = format!(
                "{}/{}",
                local_base_dir.trim_end_matches(['/', '\\']),
                r.name
            );
            let out = std::process::Command::new("git")
                .args(["-C", &path, "rev-parse", "HEAD"])
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            Some((
                r.name.clone(),
                String::from_utf8_lossy(&out.stdout).trim().to_string(),
            ))
        })
        .collect()
}

fn run_phase3_repo_task(task: Phase3RepoTask, owner: &str, base_dir: &str) -> Phase3RepoResult {
    let repo = task.repo;
    let name = repo.name.clone();
    let cat = repo.updated_at_raw.clone();
    let local_head = task.local_head;

    let needs_readme = repo.readme_ja_checked_at != cat;
    let needs_ja_badge = repo.readme_ja_badge_checked_at != local_head;
    let needs_pages = repo.pages_checked_at != cat;
    let needs_deepwiki = repo.deepwiki_checked_at != local_head;
    let needs_wf = repo.wf_checked_at != local_head;

    let (readme_ja, readme_ja_cat) = if needs_readme {
        (
            Some(check_file_exists(owner, &name, "README.ja.md")),
            cat.clone(),
        )
    } else {
        (repo.readme_ja, repo.readme_ja_checked_at.clone())
    };

    let (readme_ja_badge, readme_ja_badge_cat) = if needs_ja_badge {
        (
            Some(check_readme_ja_badge(base_dir, &name)),
            local_head.clone(),
        )
    } else {
        (
            repo.readme_ja_badge,
            repo.readme_ja_badge_checked_at.clone(),
        )
    };

    let (pages, pages_cat) = if needs_pages {
        (Some(check_pages_exists(owner, &name)), cat.clone())
    } else {
        (repo.pages, repo.pages_checked_at.clone())
    };

    let (deepwiki, deepwiki_cat) = if needs_deepwiki {
        (
            Some(check_deepwiki_exists(base_dir, &name)),
            local_head.clone(),
        )
    } else {
        (repo.deepwiki, repo.deepwiki_checked_at.clone())
    };

    let (wf_workflows, wf_cat) = if needs_wf {
        (
            Some(check_workflows(base_dir, &name, repo.cargo_install)),
            local_head.clone(),
        )
    } else {
        (repo.wf_workflows, repo.wf_checked_at.clone())
    };

    Phase3RepoResult {
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
    // Phase 1: fetch repo list
    let result = do_fetch(&config, &mut history, &tx);
    match result {
        Err(e) => {
            let _ = tx.send(FetchProgress::Done(Err(e)));
        }
        Ok((mut repos, rl)) => {
            let _ = tx.send(FetchProgress::Done(Ok((repos.clone(), rl))));

            let owner = config.owner.clone();
            let auto_update_run_dir = if config.auto_update {
                Some(config.resolved_app_run_dir())
            } else {
                None
            };
            let local_heads = collect_local_heads(&repos, &config.local_base_dir);
            let cargo_handle = spawn_background_cargo_checks(
                &repos,
                &local_heads,
                &owner,
                &config.local_base_dir,
                auto_update_run_dir.as_deref(),
                &tx,
            );

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
                for (i, (name, repo_full_name)) in pullable.iter().enumerate() {
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
                let _ = tx.send(FetchProgress::Status(String::from(
                    "Refreshing after auto-pull…",
                )));
                match do_fetch(&config, &mut history, &tx) {
                    Ok((r2, rl2)) => {
                        repos = r2;
                        let _ = tx.send(FetchProgress::Done(Ok((repos.clone(), rl2))));
                    }
                    Err(e) => {
                        let _ = tx.send(FetchProgress::Done(Err(e)));
                        return;
                    }
                }
            }

            // Phase 3:
            // - README / Pages / DeepWiki / workflows は各 checked_at が古いときだけ再確認する。
            // - cargo install 状態の確認は auto-pull / existence check とは独立して先行実行する。
            let local_heads = collect_local_heads(&repos, &config.local_base_dir);
            let phase3_tasks = build_phase3_tasks(&repos, &local_heads);

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
                            r.readme_ja = result.readme_ja;
                            r.readme_ja_checked_at = result.readme_ja_cat.clone();
                            r.readme_ja_badge = result.readme_ja_badge;
                            r.readme_ja_badge_checked_at = result.readme_ja_badge_cat.clone();
                            r.pages = result.pages;
                            r.pages_checked_at = result.pages_cat.clone();
                            r.deepwiki = result.deepwiki;
                            r.deepwiki_checked_at = result.deepwiki_cat.clone();
                            r.wf_workflows = result.wf_workflows;
                            r.wf_checked_at = result.wf_cat.clone();
                        }

                        let _ = tx.send(FetchProgress::ExistenceUpdate {
                            name: result.name.clone(),
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

            if let Ok(cargo_results) = cargo_handle.join() {
                for result in &cargo_results {
                    apply_cargo_result_to_history(&mut history, result);
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
