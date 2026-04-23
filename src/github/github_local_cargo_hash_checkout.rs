use chrono::{DateTime, SecondsFormat, Utc};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn checkout_dir_modified_at(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .unwrap_or(UNIX_EPOCH)
}

fn format_checkout_dir_modified_at(timestamp: SystemTime) -> String {
    DateTime::<Utc>::from(timestamp).to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn matches_checkout_dir_name(dir_name: &str, app_name: &str) -> bool {
    dir_name == app_name
        || dir_name
            .strip_prefix(app_name)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|suffix| {
                !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_hexdigit())
            })
}

fn unique_checkout_name_candidates(repo_checkout_name: &str, app_name: &str) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    for name in [repo_checkout_name, app_name] {
        if !name.is_empty()
            && !candidates
                .iter()
                .any(|candidate| candidate.as_str() == name)
        {
            candidates.push(name.to_string());
        }
    }
    candidates
}

/// Resolve the newest cargo git checkout sub-directory for the matched git install entry.
///
/// Returns `Some(path)` only when exactly one checkout base directory matches the repo name
/// or crate name, in that priority order,
/// and that directory contains at least one checkout sub-directory to inspect. Any
/// directory read failure, ambiguous match, or missing checkout candidate is logged via
/// `log_fn` and results in `None`.
pub(super) fn resolve_checkout_subdir(
    log_fn: &mut impl FnMut(&str),
    owner: &str,
    repo_name: &str,
    cargo_home: &str,
    repo_checkout_name: &str,
    app_name: &str,
) -> Option<PathBuf> {
    let checkouts_dir = Path::new(cargo_home).join("git").join("checkouts");
    let checkout_base_dirs: Vec<PathBuf> = match std::fs::read_dir(&checkouts_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .collect(),
        Err(err) => {
            super::super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &checkouts_dir,
                &format!("cargo checkouts ディレクトリの読み取りに失敗しました: {err}"),
            );
            return None;
        }
    };

    let checkout_name_candidates = unique_checkout_name_candidates(repo_checkout_name, app_name);
    super::super::log_cargo_check_path_result(
        log_fn,
        owner,
        repo_name,
        &checkouts_dir,
        &format!("checkout dir 名の探索候補={checkout_name_candidates:?}"),
    );

    let mut checkout_base = None;
    for candidate_name in checkout_name_candidates {
        let matches: Vec<PathBuf> = checkout_base_dirs
            .iter()
            .filter(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy())
                    .is_some_and(|name| matches_checkout_dir_name(name.as_ref(), &candidate_name))
            })
            .cloned()
            .collect();
        if matches.is_empty() {
            continue;
        }

        let checkout_candidate_names = matches
            .iter()
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>();
        super::super::log_cargo_check_path_result(
            log_fn,
            owner,
            repo_name,
            &checkouts_dir,
            &format!(
                "checkouts 配下の hash 取得候補 dir 名一覧={checkout_candidate_names:?} (探索名={candidate_name:?})"
            ),
        );

        if matches.len() > 1 {
            super::super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &checkouts_dir,
                &format!(
                    "{candidate_name:?} に対応する checkout ディレクトリが複数見つかりました: {:?}",
                    matches
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                ),
            );
            return None;
        }

        checkout_base = matches.into_iter().next();
        break;
    }

    let checkout_base = match checkout_base {
        Some(path) => path,
        None => {
            super::super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &checkouts_dir,
                &format!(
                    "{repo_checkout_name:?} / {app_name:?} に対応する checkout ディレクトリが見つかりません"
                ),
            );
            return None;
        }
    };

    let checkout_entries = match std::fs::read_dir(&checkout_base) {
        Ok(entries) => entries,
        Err(err) => {
            super::super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &checkout_base,
                &format!("checkout ディレクトリの読み取りに失敗しました: {err}"),
            );
            return None;
        }
    };
    let mut checkout_candidates: Vec<(SystemTime, PathBuf)> = checkout_entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let path = e.path();
            (checkout_dir_modified_at(&path), path)
        })
        .collect();

    checkout_candidates.sort_by(|(modified_at_a, path_a), (modified_at_b, path_b)| {
        modified_at_b
            .cmp(modified_at_a)
            .then_with(|| path_b.cmp(path_a))
    });

    if !checkout_candidates.is_empty() {
        for (index, (modified_at, path)) in checkout_candidates.iter().enumerate() {
            super::super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &checkout_base,
                &format!(
                    "更新日時順の checkout subdir 候補[{index}]={} 更新日時={}",
                    path.display(),
                    format_checkout_dir_modified_at(*modified_at)
                ),
            );
        }
    }

    let (sub_dir_modified_at, sub_dir) = match checkout_candidates.into_iter().next() {
        Some(candidate) => candidate,
        None => {
            super::super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &checkout_base,
                "checkout ディレクトリに候補となる subdir がありません",
            );
            return None;
        }
    };
    super::super::log_cargo_check_path_result(
        log_fn,
        owner,
        repo_name,
        &sub_dir,
        &format!(
            "選択した checkout ディレクトリ={} 更新日時={}",
            sub_dir.display(),
            format_checkout_dir_modified_at(sub_dir_modified_at)
        ),
    );

    Some(sub_dir)
}

#[cfg(test)]
mod tests {
    use super::{matches_checkout_dir_name, unique_checkout_name_candidates};

    #[test]
    fn unique_checkout_name_candidates_prefers_repo_name_then_crate_name() {
        assert_eq!(
            unique_checkout_name_candidates("clap-mml-play-server", "clap-mml-render-server"),
            vec!["clap-mml-play-server", "clap-mml-render-server"]
        );
    }

    #[test]
    fn unique_checkout_name_candidates_deduplicates_same_name() {
        assert_eq!(
            unique_checkout_name_candidates("own-repos-curator", "own-repos-curator"),
            vec!["own-repos-curator"]
        );
    }

    #[test]
    fn matches_checkout_dir_name_accepts_exact_name() {
        assert!(matches_checkout_dir_name(
            "own-repos-curator",
            "own-repos-curator"
        ));
    }

    #[test]
    fn matches_checkout_dir_name_accepts_hash_suffix() {
        assert!(matches_checkout_dir_name(
            "own-repos-curator-deadbeef",
            "own-repos-curator"
        ));
    }

    #[test]
    fn matches_checkout_dir_name_rejects_similar_repo_name() {
        assert!(!matches_checkout_dir_name(
            "own-repos-curator-to-hatena-deadbeef",
            "own-repos-curator"
        ));
    }

    #[test]
    fn matches_checkout_dir_name_rejects_non_hash_suffix() {
        assert!(!matches_checkout_dir_name(
            "own-repos-curator-to",
            "own-repos-curator"
        ));
    }
}
