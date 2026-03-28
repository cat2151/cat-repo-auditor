pub(super) fn fetch_remote_main_hash(
    log_fn: &mut impl FnMut(&str),
    owner: &str,
    repo_name: &str,
) -> Option<String> {
    let remote_command = super::super::format_git_ls_remote_main_command(owner, repo_name);
    super::super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!("remote のコミットハッシュ取得を開始します: コマンド={remote_command}"),
    );
    let out = std::process::Command::new("git")
        .args([
            "ls-remote",
            &format!("https://github.com/{owner}/{repo_name}.git"),
            "refs/heads/main",
        ])
        .output();
    let out = match out {
        Ok(out) => out,
        Err(err) => {
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                &format!("コマンドの起動に失敗しました: コマンド={remote_command}: {err}"),
            );
            return None;
        }
    };
    super::super::log_cargo_check_command_result(log_fn, owner, repo_name, &remote_command, &out);
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    match stdout.split_whitespace().next() {
        Some(hash) => {
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                &format!("remote のコミットハッシュを取得しました: {hash}"),
            );
            Some(hash.to_string())
        }
        _ => {
            super::super::log_cargo_check_result(
                log_fn,
                owner,
                repo_name,
                "remote main ブランチのハッシュが空です",
            );
            None
        }
    }
}
