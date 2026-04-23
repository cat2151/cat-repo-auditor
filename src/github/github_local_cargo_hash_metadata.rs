#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CargoInstallMetadata {
    pub(super) crate_name: String,
    pub(super) git_url: String,
    pub(super) repo_name: String,
    pub(super) metadata_revision: String,
}

pub(super) fn parse_cargo_install_entry(entry: &str) -> Option<CargoInstallMetadata> {
    let crate_name = entry.split_whitespace().next()?.to_string();
    let git_start = entry.find("git+")? + "git+".len();
    let source = entry[git_start..].trim();
    let source = source.strip_suffix(')').unwrap_or(source).trim();
    let (git_url, metadata_revision) = source.split_once('#')?;
    let git_url = git_url.trim();
    let metadata_revision = metadata_revision.trim().trim_end_matches(')').trim();
    if git_url.is_empty() || metadata_revision.is_empty() {
        return None;
    }

    let normalized_git_url = git_url.trim_end_matches('/').trim_end_matches(".git");
    let repo_name = normalized_git_url.rsplit('/').next()?.trim();
    if repo_name.is_empty() {
        return None;
    }

    Some(CargoInstallMetadata {
        crate_name,
        git_url: git_url.to_string(),
        repo_name: repo_name.to_string(),
        metadata_revision: metadata_revision.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_cargo_install_entry;

    #[test]
    fn parse_cargo_install_entry_extracts_crate_repo_and_revision_without_dot_git() {
        let metadata = parse_cargo_install_entry(
            "clap-mml-render-server 0.1.0 (git+https://github.com/cat2151/clap-mml-play-server#f7861234)",
        )
        .expect("metadata should parse");

        assert_eq!(metadata.crate_name, "clap-mml-render-server");
        assert_eq!(
            metadata.git_url,
            "https://github.com/cat2151/clap-mml-play-server"
        );
        assert_eq!(metadata.repo_name, "clap-mml-play-server");
        assert_eq!(metadata.metadata_revision, "f7861234");
    }

    #[test]
    fn parse_cargo_install_entry_extracts_repo_name_with_dot_git_suffix() {
        let metadata = parse_cargo_install_entry(
            "cat-edit-mml 0.1.0 (git+https://github.com/cat2151/cat-edit-mml.git#d27b5678)",
        )
        .expect("metadata should parse");

        assert_eq!(metadata.crate_name, "cat-edit-mml");
        assert_eq!(
            metadata.git_url,
            "https://github.com/cat2151/cat-edit-mml.git"
        );
        assert_eq!(metadata.repo_name, "cat-edit-mml");
        assert_eq!(metadata.metadata_revision, "d27b5678");
    }
}
