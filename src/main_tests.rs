use super::*;
use crate::main_cli::UPDATE_NOTICE_HEADER;
use crate::main_helpers::{make_log_line, make_x_log_line, STARTUP_LOG_MSG, STARTUP_LOG_SEPARATOR};
use crate::main_launch::{
    cargo_status_to_launch_args, format_launch_command, x_not_run_feedback_no_cargo_install,
};

#[test]
fn parse_subcommand_recognizes_hash() {
    let args = vec!["catrepo".to_string(), "hash".to_string()];
    assert_eq!(parse_subcommand(&args), Some(Subcommand::Hash));
}

#[test]
fn parse_subcommand_recognizes_update() {
    let args = vec!["catrepo".to_string(), "update".to_string()];
    assert_eq!(parse_subcommand(&args), Some(Subcommand::Update));
}

#[test]
fn parse_subcommand_ignores_unknown_command() {
    let args = vec!["catrepo".to_string(), "unknown".to_string()];
    assert_eq!(parse_subcommand(&args), None);
}

#[test]
fn make_x_log_line_contains_repo_and_message() {
    let line = make_x_log_line("owner/repo", "run: `bin` cwd=`.`");
    assert!(line.contains("x owner/repo run: `bin` cwd=`.`"));
}

#[test]
fn make_startup_log_line_contains_message() {
    assert!(make_startup_log_line().contains(STARTUP_LOG_MSG));
}

#[test]
fn make_log_line_contains_message() {
    assert!(make_log_line("background checks completed").contains("background checks completed"));
}

#[test]
fn startup_log_separator_matches_expected() {
    assert_eq!(STARTUP_LOG_SEPARATOR, "---");
}

#[test]
fn test_x_launch_args_cargo_ok_returns_empty_slice() {
    assert_eq!(cargo_status_to_launch_args(Some(true)), Some(&[][..]));
}

#[test]
fn test_x_launch_args_cargo_old_returns_update() {
    assert_eq!(
        cargo_status_to_launch_args(Some(false)),
        Some(&["update"][..])
    );
}

#[test]
fn test_x_launch_args_none_returns_none() {
    assert_eq!(cargo_status_to_launch_args(None), None);
}

#[test]
fn test_x_launch_display_joins_args() {
    assert_eq!(
        format_launch_command("foo.exe", &["update"]),
        "foo.exe update"
    );
}

#[test]
fn test_x_not_run_messages_match_expected_wording() {
    let (line, transient_msg) = x_not_run_feedback_no_cargo_install("owner/repo");
    assert_eq!(
        line,
        make_x_log_line("owner/repo", X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP)
    );
    assert_eq!(transient_msg, X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP);
}

#[test]
fn update_notice_header_uses_catrepo_name() {
    assert_eq!(UPDATE_NOTICE_HEADER, "catrepo update available!");
}
