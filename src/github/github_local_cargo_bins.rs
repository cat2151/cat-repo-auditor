use std::path::Path;

/// Get the installed binary names for a git-installed crate from .crates2.json.
/// Returns None if not found.
pub(crate) fn get_cargo_bins(owner: &str, repo_name: &str) -> Option<Vec<String>> {
    get_cargo_bins_inner(super::get_cargo_home(), owner, repo_name)
}

/// Internal function exposed for testing.
pub(super) fn get_cargo_bins_inner(
    cargo_home: impl AsRef<Path>,
    owner: &str,
    repo_name: &str,
) -> Option<Vec<String>> {
    let crates2_path = cargo_home.as_ref().join(".crates2.json");

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    for (key, val) in installs {
        if super::cargo_install_entry_matches_repo(key, owner, repo_name) {
            let bins = val.get("bins")?.as_array()?;
            return Some(
                bins.iter()
                    .filter_map(|b| b.as_str().map(|s| s.to_string()))
                    .collect(),
            );
        }
    }
    None
}
