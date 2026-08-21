use crate::{
    github_local::{
        append_cargo_check_after_auto_update_log, append_cargo_check_results,
        check_cargo_git_install_status, check_installed_bins, BinCheckOutcome,
        CargoGitInstallCheck,
    },
    history::History,
};

use super::{
    inspect_auto_update_after_recheck, phase3_worker_count, should_skip_auto_update_for_repo,
    AutoUpdateAfterRecheck, AutoUpdateLaunchRequest, CargoCheckFields, FetchProgress, RepoInfo,
};

/// Cargo check の状態とログ用の説明材料を保持する。
///
/// `needs_remote` は実行判定には使わず、ログで remote hash cache の状態を説明するために保持している。
#[derive(Clone, Copy)]
pub(super) struct CargoCheckStatus {
    needs_remote: bool,
}

impl CargoCheckStatus {
    pub(super) fn for_repo(repo: &RepoInfo) -> Self {
        Self {
            needs_remote: repo.cargo_remote_hash_checked_at != repo.updated_at_raw
                || repo.cargo_remote_hash.is_empty(),
        }
    }

    #[cfg(test)]
    pub(super) fn new(needs_remote: bool) -> Self {
        Self { needs_remote }
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

fn format_cargo_check_status_reason(status: CargoCheckStatus) -> &'static str {
    if status.needs_remote {
        "cargo check を実行: local check には依存せず、remote hash cache が古いか空です"
    } else {
        "cargo check を実行: local check には依存せず、installed hash 確認のため毎回実行します"
    }
}

pub(super) fn format_cargo_check_status_log(repo: &RepoInfo, status: CargoCheckStatus) -> String {
    format!(
        "{}: needs_cargo_remote={} updated_at_raw={:?} cargo_remote_hash_checked_at={:?} cargo_remote_hash_present={} cargo_install={:?} cargo_check_failed={}",
        format_cargo_check_status_reason(status),
        status.needs_remote,
        repo.updated_at_raw,
        repo.cargo_remote_hash_checked_at,
        !repo.cargo_remote_hash.is_empty(),
        repo.cargo_install,
        repo.cargo_check_failed,
    )
}

/// binary self-report を最優先に、1 repo 分の cargo check 結果フィールドを決める。
///
/// `checkout_result`（`git/checkouts` の HEAD 由来）は診断ログと local hash の取得にだけ使い、
/// ok / NG の判定には使わない。checkout HEAD は cargo が fetch した時点で remote に追いつき、
/// `~/.cargo/bin/<bin>` が置き換わったかを表さないため。
pub(super) fn resolve_cargo_check_fields(
    updated_at_raw: &str,
    bin_check: &BinCheckOutcome,
    checkout_result: &CargoGitInstallCheck,
) -> CargoCheckFields {
    // `cargo_checked_at` は local clone の HEAD を保持する診断用フィールドで、判定には使わない。
    let local_hash = match checkout_result {
        CargoGitInstallCheck::Checked { local_hash, .. } => local_hash.clone(),
        CargoGitInstallCheck::NotInstalled | CargoGitInstallCheck::Failed => String::new(),
    };

    match bin_check {
        BinCheckOutcome::UpToDate { embedded, remote }
        | BinCheckOutcome::UpdateAvailable { embedded, remote } => {
            let up_to_date = matches!(bin_check, BinCheckOutcome::UpToDate { .. });
            CargoCheckFields {
                cargo_install: Some(up_to_date),
                cargo_checked_at: local_hash,
                cargo_remote_hash: remote.clone(),
                cargo_remote_hash_checked_at: updated_at_raw.to_string(),
                cargo_installed_hash: embedded.clone(),
                cargo_check_failed: false,
                cargo_bin_check: Some(up_to_date),
            }
        }
        // check サブコマンドで確認できなかった場合は checkout HEAD へ fallback せず、
        // old / ok を断定しないまま `?` を出す。
        BinCheckOutcome::Unavailable { .. } => CargoCheckFields {
            cargo_check_failed: true,
            ..CargoCheckFields::default()
        },
        BinCheckOutcome::NotInstalled => CargoCheckFields::default(),
    }
}

/// binary self-report と checkout HEAD が食い違っていれば、その事実を伝えるログ文言を返す。
///
/// checkout dir はアプリからは削除しない（AGENTS.md）。判断材料を残すだけに留める。
pub(super) fn checkout_divergence_message(
    bin_check: &BinCheckOutcome,
    checkout_result: &CargoGitInstallCheck,
) -> Option<String> {
    let embedded = bin_check.embedded_hash()?;
    let CargoGitInstallCheck::Checked { installed_hash, .. } = checkout_result else {
        return None;
    };
    if embedded == installed_hash {
        return None;
    }
    Some(format!(
        "参考: checkout HEAD={installed_hash} は binary embedded={embedded} と食い違います。binary self-report を採用します"
    ))
}

fn log_bin_check_vs_checkout(
    owner: &str,
    repo_name: &str,
    bin_check: &BinCheckOutcome,
    checkout_result: &CargoGitInstallCheck,
) {
    if let Some(message) = checkout_divergence_message(bin_check, checkout_result) {
        append_cargo_check_results(owner, &[(repo_name.to_string(), message)]);
    }
}

pub(super) fn cargo_check_order(repos: &[RepoInfo]) -> Vec<String> {
    let mut ordered: Vec<&RepoInfo> = repos.iter().collect();
    ordered.sort_by(|a, b| b.updated_at_raw.cmp(&a.updated_at_raw));
    ordered.into_iter().map(|repo| repo.name.clone()).collect()
}

#[derive(Clone)]
struct CargoRepoTask {
    repo: RepoInfo,
}

pub(super) struct CargoRepoResult {
    pub(super) name: String,
    pub(super) full_name: String,
    pub(super) fields: CargoCheckFields,
}

fn append_auto_update_recheck_log(
    repo_full_name: &str,
    messages: impl IntoIterator<Item = impl AsRef<str>>,
) {
    append_cargo_check_after_auto_update_log(repo_full_name, messages);
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

    // installed hash の正は実行バイナリの self-report。先にこれを取る。
    let bin_check = check_installed_bins(owner, name.as_str());

    // checkout HEAD 由来の判定は診断ログ専用。cargo install 対象外なら実行する意味がない。
    let checkout_result = if matches!(bin_check, BinCheckOutcome::NotInstalled) {
        CargoGitInstallCheck::NotInstalled
    } else {
        check_cargo_git_install_status(owner, name.as_str(), base_dir)
    };
    log_bin_check_vs_checkout(owner, name.as_str(), &bin_check, &checkout_result);

    let fields = resolve_cargo_check_fields(&repo.updated_at_raw, &bin_check, &checkout_result);

    CargoRepoResult {
        name,
        full_name: repo.full_name,
        fields,
    }
}

pub(super) fn apply_cargo_result_to_history(history: &mut History, result: &CargoRepoResult) {
    if let Some(r) = history.repos.iter_mut().find(|r| r.name == result.name) {
        result.fields.apply_to(r);
    }
}

/// Spawn cargo check workers that run independently from auto-pull / existence checks.
///
/// Each completed repo sends a `FetchProgress::CargoUpdate` immediately, so the UI can reflect
/// cargo state without waiting for README / Pages / DeepWiki / workflow checks.
/// When auto update is enabled, this worker also performs the recheck-and-request flow for stale
/// cargo installs as soon as each cargo result is available.
///
/// The returned join handle yields all completed cargo results so the caller can merge them back
/// into in-memory history before saving the final history snapshot.
pub(super) fn spawn_background_cargo_checks(
    repos: &[RepoInfo],
    owner: &str,
    base_dir: &str,
    auto_update_run_dir: Option<&str>,
    tx: &std::sync::mpsc::Sender<FetchProgress>,
) -> std::thread::JoinHandle<Vec<CargoRepoResult>> {
    let cargo_check_statuses: std::collections::HashMap<String, CargoCheckStatus> = repos
        .iter()
        .map(|repo| (repo.name.clone(), CargoCheckStatus::for_repo(repo)))
        .collect();
    let cargo_check_logs: Vec<(String, String)> = repos
        .iter()
        .map(|repo| {
            let status = cargo_check_status(&cargo_check_statuses, &repo.name);
            (
                repo.name.clone(),
                format_cargo_check_status_log(repo, status),
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
        let _ = tx.send(FetchProgress::PhaseProgress {
            tag: "cgo",
            cur: 0,
            total: total_check,
        });
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
                    fields: result.fields.clone(),
                });

                if auto_update_run_dir.is_some() {
                    if should_skip_auto_update_for_repo(&owner, &result.name) {
                        append_auto_update_recheck_log(
                            &result.full_name,
                            [
                                "この repo は cat-repo-auditor 自身なので、自動 update は起動しません。",
                                "手動で `catrepo update` を実行してください。",
                            ],
                        );
                        let _ = tx.send(FetchProgress::Log(format!(
                            "x {} not run: self auto-update is disabled; run `catrepo update` manually",
                            result.full_name
                        )));
                        collected.push(result);
                        continue;
                    }

                    match inspect_auto_update_after_recheck(
                        &owner,
                        &result.name,
                        result.fields.cargo_install,
                        check_installed_bins,
                    ) {
                        AutoUpdateAfterRecheck::StillOld {
                            installed_hash,
                            remote_hash,
                        } => {
                            let _ = tx.send(FetchProgress::RequestAutoUpdateLaunch(
                                AutoUpdateLaunchRequest {
                                    name: result.name.clone(),
                                    full_name: result.full_name.clone(),
                                    cargo_install: result.fields.cargo_install,
                                    installed_hash,
                                    remote_hash,
                                },
                            ));
                        }
                        AutoUpdateAfterRecheck::UpdatedDuringRecheck {
                            installed_hash,
                            remote_hash,
                        } => {
                            append_auto_update_recheck_log(
                                &result.full_name,
                                [
                                    "この repo は cargo check で old でしたが、recheck 時点では update 実行前に installed hash が更新されていました。".to_string(),
                                    format!(
                                        "installed hash 確認結果 (binary self-report): installed_hash={installed_hash} remote_hash={remote_hash}"
                                    ),
                                    String::from(
                                        "remote hash と一致したため update サブコマンドは実行しません。",
                                    ),
                                ],
                            );
                            let _ = tx.send(FetchProgress::Log(format!(
                                "x {} not run: cargo install status changed on recheck",
                                result.full_name
                            )));
                        }
                        AutoUpdateAfterRecheck::RecheckFailed => {
                            append_auto_update_recheck_log(
                                &result.full_name,
                                [
                                    String::from(
                                        "この repo は cargo check で old でしたが、recheck で installed binary の check サブコマンドから hash を取得できませんでした。",
                                    ),
                                    String::from(
                                        "recheck 失敗のため update サブコマンドは実行せず、1分後の polling も開始しません。",
                                    ),
                                ],
                            );
                            let _ = tx.send(FetchProgress::Log(format!(
                                "x {} not run: cargo install recheck failed",
                                result.full_name
                            )));
                        }
                        AutoUpdateAfterRecheck::NotOldBeforeRecheck => {}
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
