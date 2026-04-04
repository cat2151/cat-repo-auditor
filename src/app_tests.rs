use super::*;
use crate::ui::{Focus, SearchState};

#[path = "app_test_support.rs"]
mod support;

#[path = "app_tests/detail_pane_tests.rs"]
mod detail_pane_tests;
#[path = "app_tests/log_and_workflow_tests.rs"]
mod log_and_workflow_tests;
#[path = "app_tests/num_prefix_tests.rs"]
mod num_prefix_tests;
#[path = "app_tests/cargo_hash_poll_tests.rs"]
mod cargo_hash_poll_tests;
#[path = "app_tests/repo_navigation_tests.rs"]
mod repo_navigation_tests;
#[path = "app_tests/search_tests.rs"]
mod search_tests;
