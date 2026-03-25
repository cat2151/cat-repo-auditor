use super::*;

#[test]
fn make_x_log_line_contains_repo_and_message() {
    let line = make_x_log_line("owner/repo", "run: `bin` cwd=`.`");
    assert!(line.contains("x owner/repo run: `bin` cwd=`.`"));
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
