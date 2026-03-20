use super::*;

#[test]
fn make_x_log_line_contains_repo_and_message() {
    let line = make_x_log_line("owner/repo", "run: `bin` cwd=`.`");
    assert!(line.contains("x owner/repo run: `bin` cwd=`.`"));
}

#[test]
fn x_launch_args_runs_bin_directly_for_cargo_ok() {
    assert_eq!(x_launch_args(Some(true)), Some(&[][..]));
}

#[test]
fn x_launch_args_runs_update_for_cargo_old() {
    assert_eq!(x_launch_args(Some(false)), Some(&["update"][..]));
}

#[test]
fn x_launch_args_none_for_uninstalled_repo() {
    assert_eq!(x_launch_args(None), None);
}

#[test]
fn x_launch_display_formats_command_with_args() {
    assert_eq!(x_launch_display("foo.exe", &["update"]), "foo.exe update");
}
