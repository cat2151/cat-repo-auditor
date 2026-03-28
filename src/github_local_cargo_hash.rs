use std::path::Path;
use std::process::Command;

#[path = "github_local_cargo_hash_checkout.rs"]
mod checkout;
#[path = "github_local_cargo_hash_remote.rs"]
mod remote;

/// Compare the commit hash of a `cargo install --git` entry against local HEAD.
///
/// Method:
///   1. Parse `.crates2.json` for the matching entry to get the crate (app) name only.
///   2. Find `$CARGO_HOME/git/checkouts/<app_name>` or
///      `$CARGO_HOME/git/checkouts/<app_name>-*` (exact or prefix match with "-" delimiter).
///      Multiple matches → call `log_fn` and return None.
///   3. Sort sub-directories of the checkout by modification timestamp; run `git rev-parse HEAD`
///      in the most recently modified one to obtain the installed commit hash.
///   4. Run `git ls-remote ... refs/heads/main` against the GitHub remote to obtain the
///      remote `main` hash for logging.
///   5. Run `git rev-parse HEAD` in the local clone and compare.
///
/// Returns:
///   None                         – repo not installed via `cargo install --git`, OR
///                                  .crates2.json is missing/unreadable/unparseable, OR
///                                  checkout directory not found, OR
///                                  `git rev-parse HEAD` failed, OR
///                                  `git ls-remote` failed
///   Some((true,  inst, local, remote))   – installed hash == local HEAD
///   Some((false, inst, local, remote))   – installed hash != local HEAD (stale install)
pub(crate) fn check_cargo_git_install(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
) -> Option<(bool, String, String, String)> {
    let cargo_home = super::get_cargo_home();
    check_cargo_git_install_with_resolver_and_logger(
        owner,
        repo_name,
        base_dir,
        &cargo_home,
        super::append_log_message,
        remote::fetch_remote_main_hash,
    )
}

fn check_cargo_git_install_with_resolver_and_logger<L, R>(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    mut log_fn: L,
    mut resolve_remote_hash: R,
) -> Option<(bool, String, String, String)>
where
    L: FnMut(&str),
    R: FnMut(&mut L, &str, &str) -> Option<String>,
{
    super::log_cargo_check_result(
        &mut log_fn,
        owner,
        repo_name,
        "開始: cargo check を開始します",
    );
    let result = check_cargo_git_install_inner_with_resolver(
        owner,
        repo_name,
        base_dir,
        cargo_home,
        &mut log_fn,
        &mut resolve_remote_hash,
    );
    let completion_message = match &result {
        Some((_matches_local, installed_hash, _local_hash, remote_hash)) => format!(
            "終了: cargo check を完了しました (cargo install と remote の比較結果={})",
            if installed_hash == remote_hash {
                "一致"
            } else {
                "不一致"
            }
        ),
        None => String::from("終了: cargo check を完了しました (チェック対象外または判定不能)"),
    };
    super::log_cargo_check_result(&mut log_fn, owner, repo_name, &completion_message);
    result
}

/// Internal function exposed for testing.
#[cfg(test)]
pub(super) fn check_cargo_git_install_inner(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    mut log_fn: impl FnMut(&str),
) -> Option<(bool, String, String, String)> {
    check_cargo_git_install_inner_with_resolver(
        owner,
        repo_name,
        base_dir,
        cargo_home,
        &mut log_fn,
        |log_fn, owner, repo_name| remote::fetch_remote_main_hash(log_fn, owner, repo_name),
    )
}

fn check_cargo_git_install_inner_with_resolver<L, R>(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    log_fn: &mut L,
    mut resolve_remote_hash: R,
) -> Option<(bool, String, String, String)>
where
    L: FnMut(&str),
    R: FnMut(&mut L, &str, &str) -> Option<String>,
{
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        "cargo install メタデータ内の対象リポジトリ情報の確認を開始します",
    );
    let crates2_path = Path::new(cargo_home).join(".crates2.json");
    let content = match std::fs::read_to_string(&crates2_path) {
        Ok(content) => content,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                super::log_cargo_check_path_result(
                    log_fn,
                    owner,
                    repo_name,
                    &crates2_path,
                    "cargo install メタデータファイルが見つからないため、cargo install の確認をスキップします",
                );
                return None;
            }
            super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &crates2_path,
                &format!("cargo install メタデータファイルの読み取りに失敗しました: {err}"),
            );
            return None;
        }
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(json) => json,
        Err(err) => {
            super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &crates2_path,
                &format!("cargo install メタデータファイルの解析に失敗しました: {err}"),
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
                log_fn,
                owner,
                repo_name,
                &crates2_path,
                "cargo install メタデータファイルに installs オブジェクトがありません",
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
            super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &crates2_path,
                "cargo install メタデータ内に対象リポジトリが見つからないため、cargo install の確認をスキップします",
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
                log_fn,
                owner,
                repo_name,
                "一致した cargo install エントリに crate 名が含まれていません",
            );
            return None;
        }
    };
    super::log_cargo_check_path_result(
        log_fn,
        owner,
        repo_name,
        &crates2_path,
        &format!(
            "一致した cargo install エントリ={matched_entry:?}、一致した crate 名={app_name:?}"
        ),
    );
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        "cargo install メタデータに対象リポジトリの情報があるため、cargo check の対象です",
    );

    let sub_dir =
        checkout::resolve_checkout_subdir(log_fn, owner, repo_name, cargo_home, &app_name)?;

    let installed_command = super::format_git_rev_parse_head_command(&sub_dir);
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!(
            "インストール済み checkout のコミットハッシュ取得を開始します: コマンド={installed_command}"
        ),
    );
    let out = Command::new("git")
        .arg("-C")
        .arg(&sub_dir)
        .args(["rev-parse", "HEAD"])
        .output();
    let out = match out {
        Ok(out) => out,
        Err(err) => {
            super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                &format!("コマンドの起動に失敗しました: コマンド={installed_command}: {err}"),
            );
            return None;
        }
    };
    super::log_cargo_check_command_result(log_fn, owner, repo_name, &installed_command, &out);
    if !out.status.success() {
        return None;
    }
    let installed_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if installed_hash.is_empty() {
        super::log_cargo_check_result(
            log_fn,
            owner,
            repo_name,
            "インストール済み checkout の HEAD ハッシュが空です",
        );
        return None;
    }
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!("インストール済み checkout のコミットハッシュを取得しました: {installed_hash}"),
    );
    let remote_hash = resolve_remote_hash(log_fn, owner, repo_name)?;

    let repo_path = Path::new(base_dir).join(repo_name);
    let local_command = super::format_git_rev_parse_head_command(&repo_path);
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!("ローカルリポジトリのコミットハッシュ取得を開始します: コマンド={local_command}"),
    );
    let out = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .args(["rev-parse", "HEAD"])
        .output();
    let out = match out {
        Ok(out) => out,
        Err(err) => {
            super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                &format!("コマンドの起動に失敗しました: コマンド={local_command}: {err}"),
            );
            return None;
        }
    };
    super::log_cargo_check_command_result(log_fn, owner, repo_name, &local_command, &out);
    if !out.status.success() {
        return None;
    }
    let local_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if local_hash.is_empty() {
        super::log_cargo_check_result(
            log_fn,
            owner,
            repo_name,
            "ローカルリポジトリの HEAD ハッシュが空です",
        );
        return None;
    }
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!("ローカルリポジトリのコミットハッシュを取得しました: {local_hash}"),
    );

    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &super::format_cargo_hash_summary(&remote_hash, &installed_hash, &local_hash),
    );
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!(
            "cargo install と remote の比較結果={}",
            if installed_hash == remote_hash {
                "一致"
            } else {
                "不一致"
            }
        ),
    );

    Some((
        installed_hash == local_hash,
        installed_hash,
        local_hash,
        remote_hash,
    ))
}

#[cfg(test)]
pub(super) fn check_cargo_git_install_inner_with_remote_hash(
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
            super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                "remote のコミットハッシュ取得を開始します",
            );
            super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                &format!("remote のコミットハッシュを取得しました: {remote_hash}"),
            );
            Some(remote_hash.to_string())
        },
    )
}

#[cfg(test)]
pub(super) fn check_cargo_git_install_with_remote_hash_and_logger(
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
            super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                "remote のコミットハッシュ取得を開始します",
            );
            super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                &format!("remote のコミットハッシュを取得しました: {remote_hash}"),
            );
            Some(remote_hash.to_string())
        },
    )
}
