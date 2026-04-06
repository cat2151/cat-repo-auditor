use crate::{
    github_local::{append_cargo_check_results, check_cargo_git_install},
    history::History,
    main_launch::spawn_cargo_app_for_repo,
};

use super::{phase3_worker_count, should_spawn_auto_update_after_recheck, FetchProgress, RepoInfo};

/// Cargo check の状態とログ用の説明材料を保持する。
///
/// `needs_local` / `needs_remote` は実行判定には使わず、ログで
/// 「何が最新か / 何が古いか」を説明するために保持している。
#[derive(Clone, Copy)]
pub(super) struct CargoCheckStatus {
    needs_local: bool,
    needs_remote: bool,
}

impl CargoCheckStatus {
    pub(super) fn for_repo(repo: &RepoInfo, local_head: &str) -> Self {
        Self {
            needs_local: repo.cargo_checked_at != local_head,
            needs_remote: repo.cargo_remote_hash_checked_at != repo.updated_at_raw
                || repo.cargo_remote_hash.is_empty(),
        }
    }

    #[cfg(test)]
    pub(super) fn new(needs_local: bool, needs_remote: bool) -> Self {
        Self {
            needs_local,
            needs_remote,
        }
    }

    #[cfg(test)]
    pub(super) fn needs_local(self) -> bool {
        self.needs_local
    }

    #[cfg(test)]
    pub(super) fn needs_remote(self) -> bool {
        self.needs_remote
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

pub(super) fn format_cargo_check_status_log(
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

pub(super) fn resolve_cargo_check_fields(
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

pub(super) fn cargo_check_order(repos: &[RepoInfo]) -> Vec<String> {
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
struct CargoRepoTask {
    repo: RepoInfo,
}

pub(super) struct CargoRepoResult {
    pub(super) name: String,
    pub(super) full_name: String,
    pub(super) cargo_install: Option<bool>,
    pub(super) cargo_cat: String,
    pub(super) cargo_remote_hash: String,
    pub(super) cargo_remote_hash_cat: String,
    pub(super) cargo_installed_hash: String,
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

pub(super) fn apply_cargo_result_to_history(history: &mut History, result: &CargoRepoResult) {
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
pub(super) fn spawn_background_cargo_checks(
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
            (
                repo.name.clone(),
                CargoCheckStatus::for_repo(repo, local_head),
            )
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
                        let mut work_queue = work_queue.lock().unwrap_or_else(|e| e.into_inner());
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
                        let feedback = spawn_cargo_app_for_repo(
                            &owner,
                            &result.name,
                            result.cargo_install,
                            run_dir,
                        );
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
