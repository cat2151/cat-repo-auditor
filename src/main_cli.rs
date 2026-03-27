use anyhow::Result;

pub(crate) const UPDATE_NOTICE_HEADER: &str = "catrepo update available!";

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
        println!("{UPDATE_NOTICE_HEADER}");
        println!("Run:");
        println!("cargo install --force --git https://github.com/{repo}");
        println!();
    }
    Ok(())
}
