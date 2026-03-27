use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Subcommand {
    Hash,
    Update,
}

pub(crate) fn parse_subcommand(args: &[String]) -> Option<Subcommand> {
    match args.get(1).map(String::as_str) {
        Some("hash") => Some(Subcommand::Hash),
        Some("update") => Some(Subcommand::Update),
        _ => None,
    }
}

pub(crate) fn print_update_notice(repo: Option<&str>) -> Result<()> {
    if let Some(repo) = repo {
        println!();
        println!("gh-tui update available!");
        println!("Run:");
        println!("cargo install --force --git https://github.com/{repo}");
        println!();
    }
    Ok(())
}
