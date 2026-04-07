#[path = "github_local_cargo.rs"]
mod cargo;
#[path = "github_local_checks.rs"]
mod checks;
#[path = "github_local_git.rs"]
mod git;
#[path = "github_local_launch.rs"]
mod launch;

pub(crate) use cargo::{
    append_cargo_check_after_auto_update_log, append_cargo_check_results, check_cargo_git_install,
    get_cargo_bins,
};
pub(crate) use checks::{
    check_deepwiki_exists, check_file_exists, check_pages_exists, check_readme_ja_badge,
    check_workflows, collect_workflow_repo_exist_checks,
};
pub(crate) use git::{
    check_local_status_no_fetch, git_pull, local_head_matches_upstream, WORKFLOW_SOURCE_REPO,
};
pub(crate) use launch::{launch_app_with_args, launch_lazygit, open_url, spawn_app_with_args};

#[cfg(test)]
use git::local_head_matches_upstream_with_logger;

const CALL_WORKFLOW_PREFIX: &str = "call-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowRepoExistRepo {
    pub name: String,
    pub updated_at: String,
    pub updated_at_raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowRepoExistCheck {
    pub workflow_file: String,
    pub installed_repos: Vec<WorkflowRepoExistRepo>,
    pub missing_repos: Vec<WorkflowRepoExistRepo>,
}

#[cfg(test)]
#[path = "github_local_tests.rs"]
mod tests;
