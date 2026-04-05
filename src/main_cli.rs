#[cfg(test)]
use clap::CommandFactory;
use clap::{Parser, Subcommand as ClapSubcommand};

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
