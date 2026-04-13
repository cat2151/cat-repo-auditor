use crate::github_local::{get_cargo_bins, launch_app_with_args};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchFeedback {
    pub transient_msg: String,
    pub log_msg: String,
    pub launched: bool,
}

pub(crate) fn launch_cargo_app_for_repo(
    owner: &str,
    repo_name: &str,
    cargo_install: Option<bool>,
    run_dir: &str,
) -> LaunchFeedback {
    launch_cargo_app_for_repo_with(
        owner,
        repo_name,
        cargo_install,
        run_dir,
        get_cargo_bins,
        launch_app_with_args,
    )
}

pub(crate) fn launch_cargo_app_for_repo_with<GetBins, Launch>(
    owner: &str,
    repo_name: &str,
    cargo_install: Option<bool>,
    run_dir: &str,
    get_bins: GetBins,
    launch: Launch,
) -> LaunchFeedback
where
    GetBins: FnOnce(&str, &str) -> Option<Vec<String>>,
    Launch: FnOnce(&str, &[&str], &str) -> anyhow::Result<()>,
{
    if let Some(args) = cargo_status_to_launch_args(cargo_install) {
        if let Some(bins) = get_bins(owner, repo_name) {
            if let Some(bin) = bins.first() {
                let bin = bin.clone();
                let cmd = format_launch_command(&bin, args);
                let cmd_desc = format!("run: `{cmd}` cwd=`{run_dir}`");
                match launch(&bin, args, run_dir) {
                    Ok(()) => LaunchFeedback {
                        transient_msg: format!("launched: {cmd}"),
                        log_msg: cmd_desc,
                        launched: true,
                    },
                    Err(e) => LaunchFeedback {
                        transient_msg: format!("run failed: {e}"),
                        log_msg: format!("{cmd_desc} => failed: {e}"),
                        launched: false,
                    },
                }
            } else {
                LaunchFeedback {
                    transient_msg: String::from("x: no installed cargo bin found"),
                    log_msg: String::from("not run: no installed cargo bin found"),
                    launched: false,
                }
            }
        } else {
            LaunchFeedback {
                transient_msg: String::from("x: no matching cargo install entry"),
                log_msg: String::from("not run: .crates2.json has no matching install entry"),
                launched: false,
            }
        }
    } else {
        LaunchFeedback {
            transient_msg: String::from(X_NOT_RUN_MSG_NO_CARGO_INSTALLED_APP),
            log_msg: String::from(X_NOT_RUN_LOG_NO_CARGO_INSTALLED_APP),
            launched: false,
        }
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
