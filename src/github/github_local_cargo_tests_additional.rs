use super::*;

#[test]
fn append_cargo_check_after_auto_update_log_writes_repo_section_and_messages() {
    let tmp = unique_temp_dir("cargo_after_auto_update_log");
    let log_path = tmp.join("logs").join("cargo_check_after_auto_update.log");

    append_cargo_check_after_auto_update_log_for_path(
        &log_path,
        "owner/myrepo",
        [
            "この repo は cargo check で old でしたので、update サブコマンドを実行しました。",
            "1分後から、1分間隔で installed hash を確認します。",
        ],
    );

    let persisted = std::fs::read_to_string(&log_path).unwrap();
    std::fs::remove_dir_all(&tmp).ok();

    let lines: Vec<_> = persisted.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(contains_human_readable_timestamp(lines[0]));
    assert!(lines[0].contains("========== owner/myrepo =========="));
    assert!(lines[1].contains("update サブコマンドを実行しました"));
    assert!(lines[2].contains("1分後から、1分間隔で installed hash を確認します"));
}

#[test]
fn cargo_install_returns_some_false_when_hashes_differ() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_stale_{}", std::process::id()));
    let local_repo = tmp.join("repos").join("myrepo");
    let local_hash = init_git_repo_with_content(&local_repo, "local-content");

    let sub_dir = tmp
        .join("cargo_home")
        .join("git")
        .join("checkouts")
        .join("myrepo-abc12345")
        .join("abcdef12");
    let installed_hash = init_git_repo_with_content(&sub_dir, "installed-content");
    assert_ne!(local_hash, installed_hash);

    let json = make_crates2_json("owner", "myrepo", "myrepo");
    let cargo_home = tmp.join("cargo_home");
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();
    let remote_hash = "fedcba9876543210fedcba9876543210fedcba98";

    let result = check_cargo_git_install_inner_with_remote_hash(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        remote_hash,
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc, remote) = result.expect("should return Some");
    assert!(
        !matches,
        "hashes should differ: inst={inst} remote={remote}"
    );
    assert_eq!(inst, installed_hash);
    assert_eq!(loc, local_hash);
    assert_eq!(remote, remote_hash);
}

#[test]
fn cargo_install_does_not_depend_on_local_clone_hash() {
    let tmp = unique_temp_dir("cargo_test_no_local_clone");
    let cargo_home = tmp.join("cargo_home");
    let sub_dir = cargo_home
        .join("git")
        .join("checkouts")
        .join("myrepo-abc12345")
        .join("abcdef12");
    let installed_hash = init_git_repo_with_content(&sub_dir, "installed-content");

    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner_with_remote_hash(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        &installed_hash,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc, remote) = result.expect("local clone failure must not block cargo");
    assert!(matches);
    assert_eq!(inst, installed_hash);
    assert_eq!(loc, "");
    assert_eq!(remote, installed_hash);
    assert!(logs.iter().any(|msg| {
        msg.contains("ローカルリポジトリのコミットハッシュ取得に失敗しました")
            && msg.contains("判定は継続します")
    }));
}

#[test]
fn cargo_install_picks_latest_mtime_subdir() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_mtime_{}", std::process::id()));
    let local_repo = tmp.join("repos").join("myrepo");
    let local_hash = init_git_repo_with_content(&local_repo, "local-content");

    let cargo_home = tmp.join("cargo_home");
    let checkouts = cargo_home
        .join("git")
        .join("checkouts")
        .join("myrepo-abc12345");

    let old_sub = checkouts.join("zzzzold1");
    init_git_repo_with_content(&old_sub, "old-content");

    std::thread::sleep(Duration::from_millis(1_100));

    let new_sub = checkouts.join("aaanew1");
    let expected_installed_hash = init_git_repo_with_content(&new_sub, "new-content");

    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();
    let remote_hash = "fedcba9876543210fedcba9876543210fedcba98";
    let old_sub_display = old_sub.display().to_string();
    let new_sub_display = new_sub.display().to_string();
    let mut logs = Vec::new();

    let result = check_cargo_git_install_inner_with_remote_hash(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        remote_hash,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (_matches, inst, _loc, remote) = result.expect("should return Some");
    assert_eq!(inst, expected_installed_hash);
    assert_ne!(inst, local_hash);
    assert_eq!(remote, remote_hash);
    let candidate_logs = logs
        .iter()
        .filter(|msg| msg.contains("更新日時順の checkout subdir 候補["))
        .collect::<Vec<_>>();
    assert_eq!(
        candidate_logs.len(),
        2,
        "このテストでは checkout subdir を old/new の 2 つだけ作成している"
    );
    assert!(candidate_logs.iter().any(|msg| {
        msg.contains("更新日時順の checkout subdir 候補[0]=")
            && msg.contains(&new_sub_display)
            && msg.contains("更新日時=")
            && contains_human_readable_timestamp(msg)
    }));
    assert!(candidate_logs.iter().any(|msg| {
        msg.contains("更新日時順の checkout subdir 候補[1]=")
            && msg.contains(&old_sub_display)
            && msg.contains("更新日時=")
            && contains_human_readable_timestamp(msg)
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("選択した checkout ディレクトリ=")
            && msg.contains(&new_sub_display)
            && msg.contains("更新日時=")
            && contains_human_readable_timestamp(msg)
    }));
}

#[test]
fn cargo_install_wrapper_logs_skip_reason_and_end_when_repo_is_not_target() {
    let tmp = unique_temp_dir("cargo_test_skip_log");
    let json = make_crates2_json("owner", "other-repo", "other-repo");
    let cargo_home = tmp.join("cargo_home");
    std::fs::create_dir_all(&cargo_home).unwrap();
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_with_remote_hash_and_logger(
        "owner",
        "myrepo",
        "/nonexistent",
        cargo_home.to_str().unwrap(),
        "fedcba9876543210fedcba9876543210fedcba98",
        |msg| logs.push(msg.to_string()),
    );

    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs
        .iter()
        .any(|msg| msg.contains("開始: cargo check を開始します")));
    assert!(logs.iter().any(|msg| {
        msg.contains("cargo install メタデータ内に対象リポジトリが見つからないため、cargo install の確認をスキップします")
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("終了: cargo check を完了しました") && msg.contains("チェック対象外")
    }));
}

#[test]
fn get_cargo_bins_returns_installed_bins_for_matching_repo() {
    let tmp = unique_temp_dir("cargo_test_bins");
    let json = make_crates2_json("owner", "myrepo", "catrepo");
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();

    let bins = get_cargo_bins_inner(&tmp, "owner", "myrepo");

    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(bins, Some(vec![String::from("catrepo")]));
}

#[test]
fn get_cargo_bins_matches_git_url_with_dot_git_suffix() {
    let tmp = unique_temp_dir("cargo_test_bins_dot_git");
    let json = make_crates2_json_with_dot_git("owner", "myrepo", "catrepo");
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();

    let bins = get_cargo_bins_inner(&tmp, "owner", "myrepo");

    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(bins, Some(vec![String::from("catrepo")]));
}

#[test]
fn get_cargo_bins_returns_none_when_repo_is_not_installed() {
    let tmp = unique_temp_dir("cargo_test_bins_missing");
    let json = make_crates2_json("owner", "other-repo", "catrepo");
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();

    let bins = get_cargo_bins_inner(&tmp, "owner", "myrepo");

    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(bins, None);
}
