use anyhow::Result;
#[cfg(test)]
use clap::CommandFactory;
use clap::{Parser, Subcommand as ClapSubcommand};

use crate::self_update::{install_cmd, owner_repo};

pub(crate) const UPDATE_NOTICE_HEADER: &str = "catrepo update available!";

#[derive(Parser, Debug)]
#[command(name = "catrepo")]
#[command(about = "A TUI for auditing GitHub repositories")]
struct Cli {
    #[command(subcommand)]
    command: Option<Subcommand>,
}

#[derive(ClapSubcommand, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Subcommand {
    /// Print the build-time commit hash
    Hash,
    /// Self-update the application from GitHub
    Update,
    /// Compare the build-time commit hash with the remote main branch
    Check,
}

pub(crate) fn parse_subcommand(args: &[String]) -> clap::error::Result<Option<Subcommand>> {
    Cli::try_parse_from(args).map(|cli| cli.command)
}

#[cfg(test)]
pub(crate) fn command() -> clap::Command {
    Cli::command()
}

pub(crate) fn print_update_notice(repo: Option<&str>) -> Result<()> {
    if let Some(repo) = repo {
        println!();
        println!("{UPDATE_NOTICE_HEADER}");
        println!("Run:");
        if repo == owner_repo() {
            println!("{}", install_cmd());
        } else {
            println!("cargo install --force --git https://github.com/{repo}");
        }
        println!();
    }
    Ok(())
}
