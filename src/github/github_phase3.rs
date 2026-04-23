use super::{LocalStatus, RepoInfo};
use crate::github_local::{
    check_deepwiki_exists, check_file_exists, check_local_status_no_fetch, check_pages_exists,
    check_readme_ja_badge, check_workflows, local_head_hash_no_fetch,
};

#[derive(Clone)]
pub(super) struct Phase3RepoTask {
    pub repo: RepoInfo,
}

pub(super) struct Phase3RepoResult {
    pub name: String,
    pub local_status: LocalStatus,
    pub has_local_git: bool,
    pub staging_files: Vec<String>,
    pub local_head_hash: String,
    pub readme_ja: Option<bool>,
    pub readme_ja_cat: String,
    pub readme_ja_badge: Option<bool>,
    pub readme_ja_badge_cat: String,
    pub pages: Option<bool>,
    pub pages_cat: String,
    pub deepwiki: Option<bool>,
    pub deepwiki_cat: String,
    pub wf_workflows: Option<bool>,
    pub wf_cat: String,
}

pub(super) fn build_phase3_tasks(repos: &[RepoInfo]) -> Vec<Phase3RepoTask> {
    let mut ordered = repos.to_vec();
    ordered.sort_by(|a, b| b.updated_at_raw.cmp(&a.updated_at_raw));
    ordered
        .iter()
        .map(|repo| Phase3RepoTask { repo: repo.clone() })
        .collect()
}

pub(super) fn phase3_worker_count(total_check: usize) -> usize {
    debug_assert!(total_check > 0);
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4)
        .min(total_check)
}

pub(super) fn apply_phase3_result(repo: &mut RepoInfo, result: &Phase3RepoResult) {
    repo.local_status = result.local_status.clone();
    repo.has_local_git = result.has_local_git;
    repo.staging_files = result.staging_files.clone();
    repo.local_head_hash = result.local_head_hash.clone();
    repo.readme_ja = result.readme_ja;
    repo.readme_ja_checked_at = result.readme_ja_cat.clone();
    repo.readme_ja_badge = result.readme_ja_badge;
    repo.readme_ja_badge_checked_at = result.readme_ja_badge_cat.clone();
    repo.pages = result.pages;
    repo.pages_checked_at = result.pages_cat.clone();
    repo.deepwiki = result.deepwiki;
    repo.deepwiki_checked_at = result.deepwiki_cat.clone();
    repo.wf_workflows = result.wf_workflows;
    repo.wf_checked_at = result.wf_cat.clone();
}

fn run_local_repo_task(repo: RepoInfo, base_dir: &str) -> Phase3RepoResult {
    let name = repo.name.clone();
    let (local_status, has_local_git, staging_files) = check_local_status_no_fetch(base_dir, &name);
    let local_head_hash = if has_local_git {
        local_head_hash_no_fetch(base_dir, &name)
    } else {
        String::new()
    };

    Phase3RepoResult {
        name,
        local_status,
        has_local_git,
        staging_files,
        local_head_hash,
        readme_ja: repo.readme_ja,
        readme_ja_cat: repo.readme_ja_checked_at,
        readme_ja_badge: repo.readme_ja_badge,
        readme_ja_badge_cat: repo.readme_ja_badge_checked_at,
        pages: repo.pages,
        pages_cat: repo.pages_checked_at,
        deepwiki: repo.deepwiki,
        deepwiki_cat: repo.deepwiki_checked_at,
        wf_workflows: repo.wf_workflows,
        wf_cat: repo.wf_checked_at,
    }
}

pub(super) fn spawn_background_local_checks(
    repos: &[RepoInfo],
    base_dir: &str,
    tx: &std::sync::mpsc::Sender<super::FetchProgress>,
) -> std::thread::JoinHandle<Vec<Phase3RepoResult>> {
    let tasks = build_phase3_tasks(repos);
    let base_dir = base_dir.to_string();
    let tx = tx.clone();

    std::thread::spawn(move || {
        if tasks.is_empty() {
            return vec![];
        }

        let total_check = tasks.len();
        let _ = tx.send(super::FetchProgress::PhaseProgress {
            tag: "lcl",
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
                let base_dir = base_dir.clone();
                scope.spawn(move || {
                    while let Some(task) = {
                        let mut work_queue = work_queue.lock().unwrap_or_else(|e| e.into_inner());
                        work_queue.pop_front()
                    } {
                        let result = run_local_repo_task(task.repo, &base_dir);
                        let _ = result_tx.send(result);
                    }
                });
            }
            drop(result_tx);

            for (completed, result) in result_rx.into_iter().enumerate() {
                let _ = tx.send(super::FetchProgress::PhaseProgress {
                    tag: "lcl",
                    cur: completed + 1,
                    total: total_check,
                });
                let _ = tx.send(super::FetchProgress::ExistenceUpdate {
                    name: result.name.clone(),
                    local_status: result.local_status.clone(),
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
                collected.push(result);
            }
        });

        let _ = tx.send(super::FetchProgress::PhaseProgress {
            tag: "lcl",
            cur: 0,
            total: 0,
        });
        collected
    })
}

pub(super) fn run_phase3_repo_task(
    task: Phase3RepoTask,
    owner: &str,
    base_dir: &str,
) -> Phase3RepoResult {
    let repo = task.repo;
    let name = repo.name.clone();
    let cat = repo.updated_at_raw.clone();
    let local_status = repo.local_status.clone();
    let has_local_git = repo.has_local_git;
    let staging_files = repo.staging_files.clone();
    let local_head = repo.local_head_hash.clone();

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
        local_status,
        has_local_git,
        staging_files,
        local_head_hash: local_head,
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
