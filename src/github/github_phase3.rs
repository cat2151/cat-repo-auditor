use super::{LocalStatus, RepoInfo};
use crate::github_local::{
    check_deepwiki_exists, check_file_exists, check_local_status_no_fetch, check_pages_exists,
    check_readme_ja_badge, check_workflows,
};
use std::collections::HashMap;

#[derive(Clone)]
pub(super) struct Phase3RepoTask {
    pub repo: RepoInfo,
    pub local_head: String,
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

pub(super) fn build_phase3_tasks(
    repos: &[RepoInfo],
    local_heads: &HashMap<String, String>,
) -> Vec<Phase3RepoTask> {
    repos
        .iter()
        .map(|repo| Phase3RepoTask {
            repo: repo.clone(),
            local_head: local_heads.get(&repo.name).cloned().unwrap_or_default(),
        })
        .collect()
}

pub(super) fn phase3_worker_count(total_check: usize) -> usize {
    debug_assert!(total_check > 0);
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4)
        .min(total_check)
}

pub(super) fn collect_local_heads(
    repos: &[RepoInfo],
    local_base_dir: &str,
) -> HashMap<String, String> {
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

pub(super) fn run_phase3_repo_task(
    task: Phase3RepoTask,
    owner: &str,
    base_dir: &str,
) -> Phase3RepoResult {
    let repo = task.repo;
    let name = repo.name.clone();
    let cat = repo.updated_at_raw.clone();
    let local_head = task.local_head;
    let (local_status, has_local_git, staging_files) = check_local_status_no_fetch(base_dir, &name);

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
