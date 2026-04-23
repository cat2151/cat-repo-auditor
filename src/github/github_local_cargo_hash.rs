use std::path::Path;
use std::process::Command;

#[path = "github_local_cargo_hash_checkout.rs"]
mod checkout;
#[path = "github_local_cargo_hash_remote.rs"]
mod remote;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CargoGitInstallCheck {
    NotInstalled,
    Failed,
    Checked {
        matches_remote: bool,
        installed_hash: String,
        local_hash: String,
        remote_hash: String,
    },
}

impl CargoGitInstallCheck {
    pub(crate) fn as_legacy_tuple(&self) -> Option<(bool, String, String, String)> {
        match self {
            Self::Checked {
                matches_remote,
                installed_hash,
                local_hash,
                remote_hash,
            } => Some((
                *matches_remote,
                installed_hash.clone(),
                local_hash.clone(),
                remote_hash.clone(),
            )),
            Self::NotInstalled | Self::Failed => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoInstallMetadata {
    crate_name: String,
    git_url: String,
    repo_name: String,
    metadata_revision: String,
}

fn parse_cargo_install_entry(entry: &str) -> Option<CargoInstallMetadata> {
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

/// Compare the commit hash of a `cargo install --git` entry against remote HEAD.
///
/// Method:
///   1. Parse `.crates2.json` for the matching entry to get the crate name, git repo name,
///      and Cargo metadata revision for logging.
///   2. Find `$CARGO_HOME/git/checkouts/<repo_name>` / `<repo_name>-*`, then
///      `<crate_name>` / `<crate_name>-*`.
///   3. Run `git rev-parse HEAD` in the selected checkout cache to obtain the installed
///      commit hash.
///   4. Run `git ls-remote ... refs/heads/main` against the GitHub remote to obtain the
///      remote `main` hash for logging.
///   5. Best-effort: run `git rev-parse HEAD` in the local clone for diagnostic output only.
///
/// `check_cargo_git_install` exposes the historical tuple API for callers that only need
/// checked vs unchecked. `check_cargo_git_install_status` preserves the distinction between
/// "not installed" and "failed to resolve the current hashes".
pub(crate) fn check_cargo_git_install(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
) -> Option<(bool, String, String, String)> {
    check_cargo_git_install_status(owner, repo_name, base_dir).as_legacy_tuple()
}

pub(crate) fn check_cargo_git_install_status(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
) -> CargoGitInstallCheck {
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
) -> CargoGitInstallCheck
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
        CargoGitInstallCheck::Checked {
            installed_hash,
            remote_hash,
            ..
        } => format!(
            "終了: cargo check を完了しました (cargo install と remote の比較結果={})",
            if installed_hash == remote_hash {
                "一致"
            } else {
                "不一致"
            }
        ),
        CargoGitInstallCheck::NotInstalled => {
            String::from("終了: cargo check を完了しました (チェック対象外)")
        }
        CargoGitInstallCheck::Failed => String::from("終了: cargo check を完了しました (判定不能)"),
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
    .as_legacy_tuple()
}

fn check_cargo_git_install_inner_with_resolver<L, R>(
    owner: &str,
    repo_name: &str,
    base_dir: &str,
    cargo_home: &str,
    log_fn: &mut L,
    mut resolve_remote_hash: R,
) -> CargoGitInstallCheck
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
                    "cargo install メタデータファイルが見つからないため、cargo install の確認は判定不能です",
                );
                return CargoGitInstallCheck::Failed;
            }
            super::log_cargo_check_path_result(
                log_fn,
                owner,
                repo_name,
                &crates2_path,
                &format!("cargo install メタデータファイルの読み取りに失敗しました: {err}"),
            );
            return CargoGitInstallCheck::Failed;
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
            return CargoGitInstallCheck::Failed;
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
            return CargoGitInstallCheck::Failed;
        }
    };

    let matched_entry = match installs
        .keys()
        .find(|key| super::cargo_install_entry_matches_repo(key, owner, repo_name))
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
            return CargoGitInstallCheck::NotInstalled;
        }
    };
    let install_metadata = match parse_cargo_install_entry(&matched_entry) {
        Some(metadata) => metadata,
        None => {
            super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                "一致した cargo install エントリから crate 名、git repo 名、または metadata revision を抽出できません",
            );
            return CargoGitInstallCheck::Failed;
        }
    };
    super::log_cargo_check_path_result(
        log_fn,
        owner,
        repo_name,
        &crates2_path,
        &format!(
            "一致した cargo install エントリ={matched_entry:?}、一致した crate 名={:?}、一致した git URL={:?}、一致した git repo 名={:?}、metadata revision={:?}、metadata revision source=.crates2.json",
            install_metadata.crate_name,
            install_metadata.git_url,
            install_metadata.repo_name,
            install_metadata.metadata_revision,
        ),
    );
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        "cargo install メタデータに対象リポジトリの情報があるため、cargo check の対象です",
    );

    let sub_dir = match checkout::resolve_checkout_subdir(
        log_fn,
        owner,
        repo_name,
        cargo_home,
        &install_metadata.repo_name,
        &install_metadata.crate_name,
    ) {
        Some(sub_dir) => sub_dir,
        None => return CargoGitInstallCheck::Failed,
    };

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
            return CargoGitInstallCheck::Failed;
        }
    };
    super::log_cargo_check_command_result(log_fn, owner, repo_name, &installed_command, &out);
    if !out.status.success() {
        return CargoGitInstallCheck::Failed;
    }
    let installed_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if installed_hash.is_empty() {
        super::log_cargo_check_result(
            log_fn,
            owner,
            repo_name,
            "インストール済み checkout の HEAD ハッシュが空です",
        );
        return CargoGitInstallCheck::Failed;
    }
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!("インストール済み checkout のコミットハッシュを取得しました: {installed_hash}"),
    );
    if install_metadata.metadata_revision != installed_hash {
        super::log_cargo_check_result(
            log_fn,
            owner,
            repo_name,
            &format!(
                "参考: metadata revision と checkout HEAD が一致しません: metadata revision={} checkout HEAD={} 判定には checkout HEAD を使用します",
                install_metadata.metadata_revision, installed_hash
            ),
        );
    }

    let remote_hash = match resolve_remote_hash(log_fn, owner, repo_name) {
        Some(remote_hash) => remote_hash,
        None => return CargoGitInstallCheck::Failed,
    };

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
                &format!(
                    "ローカルリポジトリのコミットハッシュ取得に失敗しましたが、cargo install と remote の判定は継続します: コマンド={local_command}: {err}"
                ),
            );
            return finish_cargo_git_install_check(
                log_fn,
                owner,
                repo_name,
                installed_hash,
                String::new(),
                remote_hash,
            );
        }
    };
    super::log_cargo_check_command_result(log_fn, owner, repo_name, &local_command, &out);
    if !out.status.success() {
        super::log_cargo_check_result(
            log_fn,
            owner,
            repo_name,
            "ローカルリポジトリのコミットハッシュ取得に失敗しましたが、cargo install と remote の判定は継続します",
        );
        return finish_cargo_git_install_check(
            log_fn,
            owner,
            repo_name,
            installed_hash,
            String::new(),
            remote_hash,
        );
    }
    let local_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if local_hash.is_empty() {
        super::log_cargo_check_result(
            log_fn,
            owner,
            repo_name,
            "ローカルリポジトリの HEAD ハッシュが空ですが、cargo install と remote の判定は継続します",
        );
        return finish_cargo_git_install_check(
            log_fn,
            owner,
            repo_name,
            installed_hash,
            String::new(),
            remote_hash,
        );
    }
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!("ローカルリポジトリのコミットハッシュを取得しました: {local_hash}"),
    );

    finish_cargo_git_install_check(
        log_fn,
        owner,
        repo_name,
        installed_hash,
        local_hash,
        remote_hash,
    )
}

fn finish_cargo_git_install_check(
    log_fn: &mut impl FnMut(&str),
    owner: &str,
    repo_name: &str,
    installed_hash: String,
    local_hash: String,
    remote_hash: String,
) -> CargoGitInstallCheck {
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

    CargoGitInstallCheck::Checked {
        matches_remote: installed_hash == remote_hash,
        installed_hash,
        local_hash,
        remote_hash,
    }
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
    .as_legacy_tuple()
}

#[cfg(test)]
pub(super) fn check_cargo_git_install_status_with_remote_failure_and_logger(
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
                "remote のコミットハッシュ取得に失敗しました",
            );
            None
        },
    )
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
    .as_legacy_tuple()
}
