use crate::main_helpers::make_x_log_line;

pub(crate) fn cargo_status_to_launch_args(
    cargo_install: Option<bool>,
) -> Option<&'static [&'static str]> {
    match cargo_install {
        Some(true) => Some(&[]),
        Some(false) => Some(&["update"]),
        None => None,
    }
}

pub(crate) fn format_launch_command(bin: &str, args: &[&str]) -> String {
    if args.is_empty() {
        bin.to_string()
    } else {
        format!("{bin} {}", args.join(" "))
    }
}

/// Persistent log message for the x-key path when no cargo-installed app is runnable.
pub(crate) const X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP: &str =
    "not run: no cargo-installed app found for this repo";
/// One-shot transient UI message for the same non-runnable x-key path.
pub(crate) const X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP: &str =
    "x: no runnable cargo-installed app for this repo";

/// Returns `(persistent_log_line, transient_ui_message)` for the x-key path
/// when the selected repo has no runnable cargo-installed app.
pub(crate) fn x_not_run_feedback_no_cargo_install(repo_full_name: &str) -> (String, String) {
    (
        make_x_log_line(repo_full_name, X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP),
        String::from(X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP),
    )
}
