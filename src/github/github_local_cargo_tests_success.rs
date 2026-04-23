use super::*;

#[test]
fn cargo_install_returns_some_true_when_hashes_match() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_match_{}", std::process::id()));
    let local_repo = tmp.join("repos").join("myrepo");
    let local_hash = init_git_repo(&local_repo);

    let cargo_home = tmp.join("cargo_home");
    let installed_sub = cargo_home
        .join("git")
        .join("checkouts")
        .join("myrepo-deadbeef")
        .join("head1234");
    let out = Cmd::new("git")
        .args([
            "clone",
            "--local",
            local_repo.to_str().unwrap(),
            installed_sub.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();
    let remote_hash = local_hash.as_str();

    let result = check_cargo_git_install_with_remote_hash_and_logger(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        remote_hash,
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc, remote) = result.expect("should return Some");
    assert!(matches, "hashes should match: inst={inst} remote={remote}");
    assert_eq!(inst, local_hash);
    assert_eq!(loc, local_hash);
    assert_eq!(remote, remote_hash);
}

#[test]
fn cargo_install_logs_hash_source_details() {
    let tmp = unique_temp_dir("cargo_test_hash_log");
    let local_repo_path = tmp.join("repos").join("myrepo");
    let local_hash = init_git_repo(&local_repo_path);

    let cargo_home = tmp.join("cargo_home");
    let installed_checkout_path = cargo_home
        .join("git")
        .join("checkouts")
        .join("myrepo-deadbeef")
        .join("head1234");
    let out = Cmd::new("git")
        .args([
            "clone",
            "--local",
            local_repo_path.to_str().unwrap(),
            installed_checkout_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let crates2_path = cargo_home.join(".crates2.json");
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(&crates2_path, &json).unwrap();
    let remote_hash = "fedcba9876543210fedcba9876543210fedcba98";

    let mut logs = Vec::new();
    let result = check_cargo_git_install_with_remote_hash_and_logger(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        remote_hash,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    let crates2_path_display = crates2_path.display().to_string();
    let installed_checkout_display = installed_checkout_path.display().to_string();
    let expected_matched_crate_name = format!("一致した crate 名={:?}", "myrepo");
    let expected_local_command = format!("git -C {} rev-parse HEAD", local_repo_path.display());
    assert!(result.is_some());
    assert!(logs
        .iter()
        .any(|msg| { msg.contains("開始: cargo check を開始します") }));
    assert!(logs.iter().any(|msg| {
        msg.contains("リポジトリ=owner/myrepo")
            && msg.contains(&crates2_path_display)
            && msg.contains("一致した cargo install エントリ=")
            && msg.contains(&expected_matched_crate_name)
            && msg.contains("一致した git repo 名=\"myrepo\"")
            && msg.contains("metadata revision=")
            && msg.contains("metadata revision source=.crates2.json")
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains(
            "cargo install メタデータに対象リポジトリの情報があるため、cargo check の対象です",
        )
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("checkouts 配下の hash 取得候補 dir 名一覧=[\"myrepo-deadbeef\"]")
    }));
    assert!(logs
        .iter()
        .any(|msg| msg.contains(&installed_checkout_display)));
    assert!(logs.iter().any(|msg| {
        msg.contains("インストール済み checkout のコミットハッシュ取得を開始します")
            && msg.contains("rev-parse HEAD")
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("コマンド=git -C")
            && msg.contains(&installed_checkout_display)
            && msg.contains("標準出力=")
            && msg.contains(&local_hash)
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains(&format!(
            "インストール済み checkout のコミットハッシュを取得しました: {local_hash}"
        ))
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("参考: metadata revision と checkout HEAD が一致しません")
            && msg.contains(DEFAULT_METADATA_REVISION)
            && msg.contains(&local_hash)
    }));
    assert!(logs
        .iter()
        .any(|msg| { msg.contains("remote のコミットハッシュ取得を開始します") }));
    assert!(logs.iter().any(|msg| {
        msg.contains(&format!(
            "remote のコミットハッシュを取得しました: {remote_hash}"
        ))
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("ローカルリポジトリのコミットハッシュ取得を開始します")
            && msg.contains(&expected_local_command)
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains(&expected_local_command)
            && msg.contains("標準出力=")
            && msg.contains(&local_hash)
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains(&format!(
            "ローカルリポジトリのコミットハッシュを取得しました: {local_hash}"
        ))
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("ハッシュ要約:")
            && msg.contains(&format!("リモートハッシュ={remote_hash},"))
            && msg.contains(&format!("cargo install ハッシュ={local_hash},"))
            && msg.contains(&format!("ローカルハッシュ={local_hash},"))
            && msg.contains("リモートと cargo install の一致=false (不一致),")
            && msg.contains("cargo install とローカルの一致=true (一致),")
            && msg.contains("リモートとローカルの一致=false (不一致)")
    }));
    assert!(logs
        .iter()
        .any(|msg| { msg.contains("cargo install と remote の比較結果=不一致") }));
    assert!(logs.iter().any(|msg| {
        msg.contains("終了: cargo check を完了しました")
            && msg.contains("cargo install と remote の比較結果=不一致")
    }));
}

#[test]
fn cargo_install_ignores_similar_checkout_dir_names() {
    let tmp = unique_temp_dir("cargo_test_similar_checkout_names");
    let local_repo = tmp.join("repos").join("own-repos-curator");
    let local_hash = init_git_repo(&local_repo);

    let cargo_home = tmp.join("cargo_home");
    let installed_sub = cargo_home
        .join("git")
        .join("checkouts")
        .join("own-repos-curator-deadbeef")
        .join("head1234");
    let out = Cmd::new("git")
        .args([
            "clone",
            "--local",
            local_repo.to_str().unwrap(),
            installed_sub.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let similar_repo = cargo_home
        .join("git")
        .join("checkouts")
        .join("own-repos-curator-to-hatena-abc88888")
        .join("head5678");
    init_git_repo_with_content(&similar_repo, "similar-repo-content");

    let json = make_crates2_json("owner", "own-repos-curator", "own-repos-curator");
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_with_remote_hash_and_logger(
        "owner",
        "own-repos-curator",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        &local_hash,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, installed_hash, local_head, remote_hash) = result.expect("should return Some");
    assert!(matches);
    assert_eq!(installed_hash, local_hash);
    assert_eq!(local_head, local_hash);
    assert_eq!(remote_hash, local_hash);
    assert!(logs.iter().any(|msg| {
        msg.contains("checkouts 配下の hash 取得候補 dir 名一覧=[\"own-repos-curator-deadbeef\"]")
    }));
    assert!(!logs
        .iter()
        .any(|msg| msg.contains("checkout ディレクトリが複数見つかりました")));
}
