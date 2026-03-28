use super::*;
use crate::github::LocalStatus;

#[path = "github_local_tests/git_pull_tests.rs"]
mod git_pull_tests;
#[path = "github_local_tests/git_status_tests.rs"]
mod git_status_tests;
#[path = "github_local_tests/helpers.rs"]
mod helpers;
#[path = "github_local_tests/repo_metadata_tests.rs"]
mod repo_metadata_tests;
