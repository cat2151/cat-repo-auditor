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

/// Resolve the newest cargo git checkout sub-directory for the matched crate name.
///
/// Returns `Some(path)` only when exactly one checkout base directory matches `app_name`
/// and that directory contains at least one checkout sub-directory to inspect. Any
/// directory read failure, ambiguous match, or missing checkout candidate is logged via
/// `log_fn` and results in `None`.
pub(super) fn resolve_checkout_subdir(
    log_fn: &mut impl FnMut(&str),
    owner: &str,
    repo_name: &str,
    cargo_home: &str,
    app_name: &str,
) -> Option<PathBuf> {
    let checkouts_dir = Path::new(cargo_home).join("git").join("checkouts");
    let prefix_with_dash = format!("{app_name}-");
    let matches: Vec<PathBuf> = match std::fs::read_dir(&checkouts_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                let name = e.file_name();
                let s = name.to_string_lossy();
                s.as_ref() == app_name || s.starts_with(prefix_with_dash.as_str())
            })
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
        &format!("checkouts 配下の hash 取得候補 dir 名一覧={checkout_candidate_names:?}"),
    );

    if matches.is_empty() {
        super::super::log_cargo_check_path_result(
            log_fn,
            owner,
            repo_name,
            &checkouts_dir,
            &format!("{app_name:?} に対応する checkout ディレクトリが見つかりません"),
        );
        return None;
    }

    if matches.len() > 1 {
        super::super::log_cargo_check_path_result(
            log_fn,
            owner,
            repo_name,
            &checkouts_dir,
            &format!(
                "{app_name:?} に対応する checkout ディレクトリが複数見つかりました: {:?}",
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
            super::super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &checkouts_dir,
                &format!(
                    "絞り込み後に {app_name:?} の checkout ディレクトリが見つかりません (内部不整合)"
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
