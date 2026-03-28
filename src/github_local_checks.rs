use crate::github::RepoInfo;
use anyhow::{anyhow, Context, Result};
use std::fs;

use super::{
    WorkflowRepoExistCheck, WorkflowRepoExistRepo, CALL_WORKFLOW_PREFIX, WORKFLOW_SOURCE_REPO,
};

/// Check if README.ja.md exists in the default branch root
pub(crate) fn check_file_exists(owner: &str, repo: &str, path: &str) -> bool {
    let endpoint = format!("/repos/{owner}/{repo}/contents/{path}");
    let out = std::process::Command::new("gh")
        .args(["api", &endpoint, "--silent"])
        .output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Check if GitHub Pages is enabled for the repo
pub(crate) fn check_pages_exists(owner: &str, repo: &str) -> bool {
    let endpoint = format!("/repos/{owner}/{repo}/pages");
    let out = std::process::Command::new("gh")
        .args(["api", &endpoint, "--silent"])
        .output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Check if a DeepWiki link is configured for the repository.
/// Scans local README.ja.md and README.md for a deepwiki.com link.
/// Returns true if "deepwiki.com" appears in either file.
pub(crate) fn check_deepwiki_exists(base_dir: &str, repo_name: &str) -> bool {
    for filename in &["README.ja.md", "README.md"] {
        let path = format!(
            "{}/{}/{}",
            base_dir.trim_end_matches(['/', '\\']),
            repo_name,
            filename
        );
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("deepwiki.com") {
                return true;
            }
        }
    }
    false
}

const REQUIRED_WORKFLOWS: [&str; 3] = [
    "call-translate-readme.yml",
    "call-issue-note.yml",
    "call-check-large-files.yml",
];
const RUST_CARGO_CHECK_WORKFLOW: &str = "call-rust-windows-cargo-check.yml";

/// Check if required workflow yml files are present in .github/workflows/
pub(crate) fn check_workflows(
    base_dir: &str,
    repo_name: &str,
    cargo_install: Option<bool>,
) -> bool {
    let base = base_dir.trim_end_matches(['/', '\\']);
    let wf_dir = format!("{}/{}/.github/workflows", base, repo_name);
    REQUIRED_WORKFLOWS
        .iter()
        .all(|f| std::path::Path::new(&format!("{}/{}", wf_dir, f)).exists())
        && (cargo_install.is_none()
            || std::path::Path::new(&format!("{}/{}", wf_dir, RUST_CARGO_CHECK_WORKFLOW)).exists())
}

pub(crate) fn collect_workflow_repo_exist_checks(
    base_dir: &str,
    repos: &[RepoInfo],
) -> Result<Vec<WorkflowRepoExistCheck>> {
    let base = base_dir.trim_end_matches(['/', '\\']);
    let workflow_dir = format!("{base}/{WORKFLOW_SOURCE_REPO}/.github/workflows");
    let workflow_dir_path = std::path::Path::new(&workflow_dir);
    if !workflow_dir_path.exists() {
        return Err(anyhow!(
            "{WORKFLOW_SOURCE_REPO}/.github/workflows が見つかりません: {workflow_dir}"
        ));
    }

    let mut workflow_files = fs::read_dir(workflow_dir_path)
        .with_context(|| format!("Failed to read workflow dir: {workflow_dir}"))?
        .map(|entry| -> Result<Option<String>> {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                return Ok(None);
            }
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                return Ok(None);
            };
            let is_call_workflow = file_name.starts_with(CALL_WORKFLOW_PREFIX)
                && (file_name.ends_with(".yml") || file_name.ends_with(".yaml"));
            Ok(is_call_workflow.then(|| file_name.to_string()))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    workflow_files.sort();

    let mut local_repos = repos
        .iter()
        .filter(|repo| repo.has_local_git && repo.name.as_str() != WORKFLOW_SOURCE_REPO)
        .map(|repo| WorkflowRepoExistRepo {
            name: repo.name.clone(),
            updated_at: repo.updated_at.clone(),
            updated_at_raw: repo.updated_at_raw.clone(),
        })
        .collect::<Vec<_>>();
    local_repos.sort_by(|a, b| {
        b.updated_at_raw
            .cmp(&a.updated_at_raw)
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(workflow_files
        .into_iter()
        .map(|workflow_file| {
            let mut installed_repos = Vec::new();
            let mut missing_repos = Vec::new();
            for repo in &local_repos {
                let path = format!("{base}/{}/.github/workflows/{workflow_file}", repo.name);
                if std::path::Path::new(&path).exists() {
                    installed_repos.push(repo.clone());
                } else {
                    missing_repos.push(repo.clone());
                }
            }
            WorkflowRepoExistCheck {
                workflow_file,
                installed_repos,
                missing_repos,
            }
        })
        .collect())
}

/// Scan local README.ja.md for a self-referencing badge/link ("README.ja.md" text).
pub(crate) fn check_readme_ja_badge(base_dir: &str, repo_name: &str) -> bool {
    for filename in &["README.ja.md", "README.md"] {
        let path = format!(
            "{}/{}/{}",
            base_dir.trim_end_matches(['/', '\\']),
            repo_name,
            filename
        );
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("README.ja.md") {
                return true;
            }
        }
    }
    false
}
