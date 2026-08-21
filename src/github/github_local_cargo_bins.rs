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

/// Get every installed binary name for a git-installed repo from .crates2.json.
///
/// Unlike [`get_cargo_bins`], which stops at the first matching install entry, this collects
/// the bins of *all* entries pointing at the repo. A cargo workspace can install several
/// packages from one repository (for example `clap-mml-play-server` installs both
/// `clap-mml-render-server` and `clap-mml-realtime-play-server`), and checking only the first
/// entry would miss a binary that stayed stale.
///
/// Returns `None` when the repo has no install entry at all.
pub(crate) fn get_cargo_bins_all(owner: &str, repo_name: &str) -> Option<Vec<String>> {
    get_cargo_bins_all_inner(super::get_cargo_home(), owner, repo_name)
}

/// Internal function exposed for testing.
pub(super) fn get_cargo_bins_all_inner(
    cargo_home: impl AsRef<Path>,
    owner: &str,
    repo_name: &str,
) -> Option<Vec<String>> {
    let crates2_path = cargo_home.as_ref().join(".crates2.json");

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    let mut matched_entry = false;
    let mut collected: Vec<String> = Vec::new();
    for (key, val) in installs {
        if !super::cargo_install_entry_matches_repo(key, owner, repo_name) {
            continue;
        }
        matched_entry = true;
        let Some(bins) = val.get("bins").and_then(|bins| bins.as_array()) else {
            continue;
        };
        for bin in bins.iter().filter_map(|b| b.as_str()) {
            if !collected.iter().any(|known| known == bin) {
                collected.push(bin.to_string());
            }
        }
    }

    matched_entry.then_some(collected)
}
