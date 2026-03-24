use std::path::Path;
use std::process::Command;

/// Extract the git source hash from a `.crates2.json` install entry key such as
/// `myrepo 0.1.0 (git+https://github.com/owner/myrepo#<hash>)`.
///
/// Returns `Some(<hash>)` when the trailing `#<hash>` portion is present and non-empty.
/// Returns `None` when the entry does not contain a usable git hash.
pub(super) fn cargo_install_source_hash(matched_entry: &str) -> Option<&str> {
    matched_entry
        .rsplit_once('#')
        .map(|(_, hash)| hash.trim_end_matches(')'))
        .filter(|hash| !hash.is_empty())
}

/// Compare the commit hash of a `cargo install --git` entry against local HEAD.
///
/// Method:
///   1. Parse `.crates2.json` for the matching entry to get the crate (app) name.
///   2. Find `$CARGO_HOME/git/checkouts/<app_name>-*` (prefix match with "-" delimiter).
///      Multiple matches → call `log_fn` and return None.
///   3. Sort sub-directories of the checkout by modification timestamp; run `git rev-parse HEAD`
///      in the most recently modified one to obtain the installed commit hash.
///   4. Run `git rev-parse HEAD` in the local clone and compare.
///
/// Returns:
///   None                         – repo not installed via `cargo install --git`, OR
///                                  .crates2.json is missing/unreadable/unparseable, OR
///                                  checkout directory not found, OR
///                                  `git rev-parse HEAD` failed
///   Some((true,  inst, local))   – installed hash == local HEAD
///   Some((false, inst, local))   – installed hash != local HEAD (stale install)
pub(crate) fn check_cargo_git_install(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
) -> Option<(bool, String, String)> {
    check_cargo_git_install_inner(owner, repo_name, base_dir, &super::get_cargo_home(), |msg| {
        super::append_error_log(msg)
    })
}

/// Internal function exposed for testing.
pub(super) fn check_cargo_git_install_inner(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    mut log_fn: impl FnMut(&str),
) -> Option<(bool, String, String)> {
    let crates2_path = Path::new(cargo_home).join(".crates2.json");
    super::log_cargo_check_path_result(
        &mut log_fn,
        owner,
        repo_name,
        &crates2_path,
        "cargo install metadata file",
    );

    let content = match std::fs::read_to_string(&crates2_path) {
        Ok(content) => content,
        Err(err) => {
            super::log_cargo_check_path_result(
                &mut log_fn,
                owner,
                repo_name,
                &crates2_path,
                &format!("failed to read cargo install metadata file: {err}"),
            );
            return None;
        }
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(json) => json,
        Err(err) => {
            super::log_cargo_check_path_result(
                &mut log_fn,
                owner,
                repo_name,
                &crates2_path,
                &format!("failed to parse cargo install metadata file: {err}"),
            );
            return None;
        }
    };
    let installs = match json
        .get("installs")
        .and_then(|installs| installs.as_object())
    {
        Some(installs) => installs,
        None => {
            super::log_cargo_check_path_result(
                &mut log_fn,
                owner,
                repo_name,
                &crates2_path,
                "cargo install metadata file does not contain installs object",
            );
            return None;
        }
    };

    let needle = format!("git+https://github.com/{owner}/{repo_name}#");

    let matched_entry = match installs
        .keys()
        .find(|key| key.trim_end_matches(')').contains(needle.as_str()))
    {
        Some(entry) => entry.to_string(),
        None => {
            super::log_cargo_check_result(
                &mut log_fn,
                owner,
                repo_name,
                "no cargo install entry matched repository",
            );
            return None;
        }
    };
    let app_name = match matched_entry
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
    {
        Some(app_name) => app_name,
        None => {
            super::log_cargo_check_result(
                &mut log_fn,
                owner,
                repo_name,
                "matched cargo install entry did not contain crate name",
            );
            return None;
        }
    };
    let metadata_hash = cargo_install_source_hash(&matched_entry).unwrap_or("<missing>");
    super::log_cargo_check_path_result(
        &mut log_fn,
        owner,
        repo_name,
        &crates2_path,
        &format!(
            "matched install entry={matched_entry:?}, matched crate name={app_name:?}, metadata hash={metadata_hash}"
        ),
    );

    let checkouts_dir = Path::new(cargo_home).join("git").join("checkouts");
    let prefix_with_dash = format!("{app_name}-");
    let matches: Vec<std::path::PathBuf> = match std::fs::read_dir(&checkouts_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                let name = e.file_name();
                let s = name.to_string_lossy();
                s.as_ref() == app_name.as_str() || s.starts_with(prefix_with_dash.as_str())
            })
            .map(|e| e.path())
            .collect(),
        Err(err) => {
            super::log_cargo_check_path_result(
                &mut log_fn,
                owner,
                repo_name,
                &checkouts_dir,
                &format!("failed to read cargo checkouts dir: {err}"),
            );
            return None;
        }
    };

    if matches.is_empty() {
        super::log_cargo_check_path_result(
            &mut log_fn,
            owner,
            repo_name,
            &checkouts_dir,
            &format!("no checkout dir found for {app_name:?}"),
        );
        return None;
    }

    if matches.len() > 1 {
        super::log_cargo_check_path_result(
            &mut log_fn,
            owner,
            repo_name,
            &checkouts_dir,
            &format!(
                "multiple checkouts found for {app_name:?}: {:?}",
                matches
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
            ),
        );
        return None;
    }

    let checkout_base = match matches.into_iter().next() {
        Some(path) => path,
        None => {
            super::log_cargo_check_path_result(
                &mut log_fn,
                owner,
                repo_name,
                &checkouts_dir,
                &format!(
                    "no checkout dir found for {app_name:?} after filtering (internal inconsistency)"
                ),
            );
            return None;
        }
    };

    let checkout_entries = match std::fs::read_dir(&checkout_base) {
        Ok(entries) => entries,
        Err(err) => {
            super::log_cargo_check_path_result(
                &mut log_fn,
                owner,
                repo_name,
                &checkout_base,
                &format!("failed to read checkout directory: {err}"),
            );
            return None;
        }
    };
    let sub_dir = match checkout_entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let mtime = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            (mtime, e.path())
        })
        .max_by(|(mt_a, pa), (mt_b, pb)| mt_a.cmp(mt_b).then_with(|| pa.cmp(pb)))
        .map(|(_, path)| path)
    {
        Some(sub_dir) => sub_dir,
        None => {
            super::log_cargo_check_path_result(
                &mut log_fn,
                owner,
                repo_name,
                &checkout_base,
                "checkout directory did not contain any candidate subdirectories",
            );
            return None;
        }
    };
    super::log_cargo_check_path_result(
        &mut log_fn,
        owner,
        repo_name,
        &checkouts_dir,
        &format!("selected checkout dir={}", sub_dir.display()),
    );

    let installed_command = super::format_git_rev_parse_head_command(&sub_dir);
    let out = Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .args(["rev-parse", "HEAD"])
        .output();
    let out = match out {
        Ok(out) => out,
        Err(err) => {
            super::log_cargo_check_result(
                &mut log_fn,
                owner,
                repo_name,
                &format!("failed to spawn command={installed_command}: {err}"),
            );
            return None;
        }
    };
    super::log_cargo_check_command_result(&mut log_fn, owner, repo_name, &installed_command, &out);
    if !out.status.success() {
        return None;
    }
    let installed_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if installed_hash.is_empty() {
        super::log_cargo_check_result(
            &mut log_fn,
            owner,
            repo_name,
            "installed checkout HEAD hash was empty",
        );
        return None;
    }

    let repo_path = Path::new(base_dir).join(repo_name);
    let local_command = super::format_git_rev_parse_head_command(&repo_path);
    let out = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .args(["rev-parse", "HEAD"])
        .output();
    let out = match out {
        Ok(out) => out,
        Err(err) => {
            super::log_cargo_check_result(
                &mut log_fn,
                owner,
                repo_name,
                &format!("failed to spawn command={local_command}: {err}"),
            );
            return None;
        }
    };
    super::log_cargo_check_command_result(&mut log_fn, owner, repo_name, &local_command, &out);
    if !out.status.success() {
        return None;
    }
    let local_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if local_hash.is_empty() {
        super::log_cargo_check_result(
            &mut log_fn,
            owner,
            repo_name,
            "local repository HEAD hash was empty",
        );
        return None;
    }

    super::log_cargo_check_result(
        &mut log_fn,
        owner,
        repo_name,
        &super::format_cargo_hash_summary(metadata_hash, &installed_hash, &local_hash),
    );

    Some((installed_hash == local_hash, installed_hash, local_hash))
}
