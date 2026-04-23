use super::{
    check_cargo_git_install_inner_with_resolver, check_cargo_git_install_with_resolver_and_logger,
    CargoGitInstallCheck,
};

pub(in super::super) fn check_cargo_git_install_inner_with_remote_hash(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    remote_hash: &str,
    mut log_fn: impl FnMut(&str),
) -> Option<(bool, String, String, String)> {
    check_cargo_git_install_inner_with_resolver(
        owner,
        repo_name,
        base_dir,
        cargo_home,
        &mut log_fn,
        |log_fn, owner, repo_name| {
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                "remote のコミットハッシュ取得を開始します",
            );
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                &format!("remote のコミットハッシュを取得しました: {remote_hash}"),
            );
            Some(remote_hash.to_string())
        },
    )
    .as_legacy_tuple()
}

pub(in super::super) fn check_cargo_git_install_status_with_remote_failure_and_logger(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    mut log_fn: impl FnMut(&str),
) -> CargoGitInstallCheck {
    check_cargo_git_install_inner_with_resolver(
        owner,
        repo_name,
        base_dir,
        cargo_home,
        &mut log_fn,
        |log_fn, owner, repo_name| {
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                "remote のコミットハッシュ取得を開始します",
            );
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                "remote のコミットハッシュ取得に失敗しました",
            );
            None
        },
    )
}

pub(in super::super) fn check_cargo_git_install_with_remote_hash_and_logger(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    remote_hash: &str,
    log_fn: impl FnMut(&str),
) -> Option<(bool, String, String, String)> {
    check_cargo_git_install_with_resolver_and_logger(
        owner,
        repo_name,
        base_dir,
        cargo_home,
        log_fn,
        |log_fn, owner, repo_name| {
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                "remote のコミットハッシュ取得を開始します",
            );
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                &format!("remote のコミットハッシュを取得しました: {remote_hash}"),
            );
            Some(remote_hash.to_string())
        },
    )
    .as_legacy_tuple()
}
