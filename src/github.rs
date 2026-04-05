use crate::{
    config::Config,
    github_fetch::do_fetch,
    github_local::{
        append_cargo_check_results, check_cargo_git_install, check_deepwiki_exists,
        check_file_exists, check_pages_exists, check_readme_ja_badge, check_workflows, git_pull,
        local_head_matches_upstream,
    },
    history::History,
    main_launch::spawn_cargo_app_for_repo,
};

#[path = "github_types.rs"]
mod types;

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

/// Cargo check の状態とログ用の説明材料を保持する。
///
/// `needs_local` / `needs_remote` は実行判定には使わず、ログで
/// 「何が最新か / 何が古いか」を説明するために保持している。
#[derive(Clone, Copy)]
struct CargoCheckStatus {
    needs_local: bool,
    needs_remote: bool,
}

impl CargoCheckStatus {
    fn for_repo(repo: &RepoInfo, local_head: &str) -> Self {
        Self {
            needs_local: repo.cargo_checked_at != local_head,
            needs_remote: repo.cargo_remote_hash_checked_at != repo.updated_at_raw
                || repo.cargo_remote_hash.is_empty(),
        }
    }
}

fn cargo_check_status(
    cargo_check_statuses: &std::collections::HashMap<String, CargoCheckStatus>,
    repo_name: &str,
) -> CargoCheckStatus {
    cargo_check_statuses
        .get(repo_name)
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "repo '{repo_name}' のcargo状態が見つかりません。すべてのrepoに状態が存在する想定です"
            )
        })
}

fn local_head_for<'a>(
    local_heads: &'a std::collections::HashMap<String, String>,
    repo_name: &str,
) -> &'a str {
    local_heads.get(repo_name).map(|s| s.as_str()).unwrap_or("")
}

fn format_cargo_check_status_reason(status: CargoCheckStatus) -> &'static str {
    match (status.needs_local, status.needs_remote) {
        (false, false) => {
            "cargo check を実行: local HEAD と remote hash cache は最新ですが、installed hash 確認のため毎回実行します"
        }
        (false, true) => {
            "cargo check を実行: local HEAD cache は最新ですが、remote hash cache が古いか空です"
        }
        (true, false) => {
            "cargo check を実行: remote hash cache は最新ですが、local HEAD cache が古いです"
        }
        (true, true) => {
            "cargo check を実行: local HEAD cache と remote hash cache の両方が古いか空です"
        }
    }
}

fn format_cargo_check_status_log(
    repo: &RepoInfo,
    local_head: &str,
    status: CargoCheckStatus,
) -> String {
    format!(
        "{}: needs_cargo_local={} needs_cargo_remote={} local_head={:?} cargo_checked_at={:?} updated_at_raw={:?} cargo_remote_hash_checked_at={:?} cargo_remote_hash_present={} cargo_install={:?}",
        format_cargo_check_status_reason(status),
        status.needs_local,
        status.needs_remote,
        local_head,
        repo.cargo_checked_at,
        repo.updated_at_raw,
        repo.cargo_remote_hash_checked_at,
        !repo.cargo_remote_hash.is_empty(),
        repo.cargo_install,
    )
}

fn resolve_cargo_check_fields(
    repo: &RepoInfo,
    updated_at_raw: &str,
    cargo_result: Option<(bool, String, String, String)>,
) -> (Option<bool>, String, String, String, String) {
    match cargo_result {
        // `loc`（git から実際に読んだ hash）を cargo_cat に使い、
        // 保存値が常に比較に使った正確な hash になるようにする。
        Some((ok, inst, loc, remote)) => (Some(ok), loc, remote, updated_at_raw.to_string(), inst),
        None => (
            repo.cargo_install,
            repo.cargo_checked_at.clone(),
            repo.cargo_remote_hash.clone(),
            repo.cargo_remote_hash_checked_at.clone(),
            repo.cargo_installed_hash.clone(),
        ),
    }
}

fn cargo_check_order(repos: &[RepoInfo]) -> Vec<String> {
    let mut ordered: Vec<&RepoInfo> = repos.iter().collect();
    ordered.sort_by_key(cargo_check_priority);
    ordered.into_iter().map(|repo| repo.name.clone()).collect()
}

fn cargo_check_priority(repo: &&RepoInfo) -> u8 {
    if repo.cargo_install == Some(false) {
        0
    } else {
        1
    }
}

#[derive(Clone)]
struct Phase3RepoTask {
    repo: RepoInfo,
    local_head: String,
}

#[derive(Clone)]
struct CargoRepoTask {
    repo: RepoInfo,
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

struct CargoRepoResult {
    name: String,
    full_name: String,
    cargo_install: Option<bool>,
    cargo_cat: String,
    cargo_remote_hash: String,
    cargo_remote_hash_cat: String,
    cargo_installed_hash: String,
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

fn build_cargo_tasks(repos: &[RepoInfo]) -> Vec<CargoRepoTask> {
    let repos_by_name: std::collections::HashMap<&str, &RepoInfo> = repos
        .iter()
        .map(|repo| (repo.name.as_str(), repo))
        .collect();
    cargo_check_order(repos)
        .into_iter()
        .filter_map(|name| {
            repos_by_name.get(name.as_str()).map(|repo| CargoRepoTask {
                repo: (*repo).clone(),
            })
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
    repos.iter()
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

fn run_cargo_repo_task(task: CargoRepoTask, owner: &str, base_dir: &str) -> CargoRepoResult {
    let repo = task.repo;
    let name = repo.name.clone();
    let (cargo_install, cargo_cat, cargo_remote_hash, cargo_remote_hash_cat, cargo_installed_hash) =
        resolve_cargo_check_fields(
            &repo,
            &repo.updated_at_raw,
            check_cargo_git_install(owner, name.as_str(), base_dir),
        );

    CargoRepoResult {
        name,
        full_name: repo.full_name,
        cargo_install,
        cargo_cat,
        cargo_remote_hash,
        cargo_remote_hash_cat,
        cargo_installed_hash,
    }
}

fn apply_cargo_result_to_history(history: &mut History, result: &CargoRepoResult) {
    if let Some(r) = history.repos.iter_mut().find(|r| r.name == result.name) {
        r.cargo_install = result.cargo_install;
        r.cargo_checked_at = result.cargo_cat.clone();
        r.cargo_remote_hash = result.cargo_remote_hash.clone();
        r.cargo_remote_hash_checked_at = result.cargo_remote_hash_cat.clone();
        r.cargo_installed_hash = result.cargo_installed_hash.clone();
    }
}

/// Spawn cargo check workers that run independently from auto-pull / existence checks.
///
/// Each completed repo sends a `FetchProgress::CargoUpdate` immediately, so the UI can reflect
/// cargo state without waiting for README / Pages / DeepWiki / workflow checks.
/// When auto update is enabled, this worker also performs the recheck-and-spawn flow for stale
/// cargo installs as soon as each cargo result is available.
///
/// The returned join handle yields all completed cargo results so the caller can merge them back
/// into in-memory history before saving the final history snapshot.
fn spawn_background_cargo_checks(
    repos: &[RepoInfo],
    local_heads: &std::collections::HashMap<String, String>,
    owner: &str,
    base_dir: &str,
    auto_update_run_dir: Option<&str>,
    tx: &std::sync::mpsc::Sender<FetchProgress>,
) -> std::thread::JoinHandle<Vec<CargoRepoResult>> {
    let cargo_check_statuses: std::collections::HashMap<String, CargoCheckStatus> = repos
        .iter()
        .map(|repo| {
            let local_head = local_head_for(local_heads, &repo.name);
            (repo.name.clone(), CargoCheckStatus::for_repo(repo, local_head))
        })
        .collect();
    let cargo_check_logs: Vec<(String, String)> = repos
        .iter()
        .map(|repo| {
            let local_head = local_head_for(local_heads, &repo.name);
            let status = cargo_check_status(&cargo_check_statuses, &repo.name);
            (
                repo.name.clone(),
                format_cargo_check_status_log(repo, local_head, status),
            )
        })
        .collect();
    append_cargo_check_results(owner, &cargo_check_logs);

    let tasks = build_cargo_tasks(repos);
    let owner = owner.to_string();
    let base_dir = base_dir.to_string();
    let auto_update_run_dir = auto_update_run_dir.map(ToOwned::to_owned);
    let tx = tx.clone();

    std::thread::spawn(move || {
        if tasks.is_empty() {
            return vec![];
        }

        let total_check = tasks.len();
        let worker_count = phase3_worker_count(total_check);
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let work_queue = std::sync::Arc::new(std::sync::Mutex::new(
            tasks.into_iter().collect::<std::collections::VecDeque<_>>(),
        ));
        let mut collected = Vec::with_capacity(total_check);

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let work_queue = std::sync::Arc::clone(&work_queue);
                let result_tx = result_tx.clone();
                let owner = owner.clone();
                let base_dir = base_dir.clone();
                scope.spawn(move || {
                    while let Some(task) = {
                        let mut work_queue =
                            work_queue.lock().unwrap_or_else(|e| e.into_inner());
                        work_queue.pop_front()
                    } {
                        let result = run_cargo_repo_task(task, &owner, &base_dir);
                        let _ = result_tx.send(result);
                    }
                });
            }
            drop(result_tx);

            for (completed, result) in result_rx.into_iter().enumerate() {
                let _ = tx.send(FetchProgress::PhaseProgress {
                    tag: "cgo",
                    cur: completed + 1,
                    total: total_check,
                });
                let _ = tx.send(FetchProgress::CargoUpdate {
                    name: result.name.clone(),
                    cargo_install: result.cargo_install,
                    cargo_cat: result.cargo_cat.clone(),
                    cargo_remote_hash: result.cargo_remote_hash.clone(),
                    cargo_remote_hash_cat: result.cargo_remote_hash_cat.clone(),
                    cargo_installed_hash: result.cargo_installed_hash.clone(),
                });

                if let Some(run_dir) = auto_update_run_dir.as_deref() {
                    if should_spawn_auto_update_after_recheck(
                        &owner,
                        &result.name,
                        &base_dir,
                        result.cargo_install,
                        check_cargo_git_install,
                    ) {
                        let feedback =
                            spawn_cargo_app_for_repo(&owner, &result.name, result.cargo_install, run_dir);
                        let _ = tx.send(FetchProgress::Log(format!(
                            "x {} {}",
                            result.full_name, feedback.log_msg
                        )));
                    } else if result.cargo_install == Some(false) {
                        let _ = tx.send(FetchProgress::Log(format!(
                            "x {} not run: cargo install status changed on recheck",
                            result.full_name
                        )));
                    }
                }

                collected.push(result);
            }
        });

        let _ = tx.send(FetchProgress::PhaseProgress {
            tag: "cgo",
            cur: 0,
            total: 0,
        });
        collected
    })
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
