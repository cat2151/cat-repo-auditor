use super::{
    append_cargo_check_after_auto_update_log_for_path, check_cargo_git_install_inner,
    check_cargo_git_install_inner_with_remote_hash,
    check_cargo_git_install_with_remote_hash_and_logger, get_cargo_bins_inner,
};
use std::process::Command as Cmd;
use std::time::Duration;

#[path = "github_local_cargo_tests_success.rs"]
mod success_tests;

fn make_crates2_json(owner: &str, repo: &str, crate_name: &str) -> String {
    make_crates2_json_with_repo_url(owner, repo, crate_name, &format!("{repo}#"))
}

fn make_crates2_json_with_dot_git(owner: &str, repo: &str, crate_name: &str) -> String {
    make_crates2_json_with_repo_url(owner, repo, crate_name, &format!("{repo}.git#"))
}

fn make_crates2_json_with_repo_url(
    owner: &str,
    _repo: &str,
    crate_name: &str,
    repo_url_suffix: &str,
) -> String {
    let key = format!(
        "{crate_name} 0.1.0 (git+https://github.com/{owner}/{repo_url_suffix}0123456789abcdef0123456789abcdef01234567)"
    );
    format!(
        "{{\"installs\":{{\"{key}\":{{\"version_req\":null,\"bins\":[\"{crate_name}\"],\
\"features\":[],\"all_features\":false,\"no_default_features\":false,\
\"profile\":\"release\",\"target\":\"x86_64-unknown-linux-gnu\",\
\"rustc\":\"rustc 1.80.0\",\"deps\":[]}}}}}}"
    )
}

fn init_git_repo_with_content(path: &std::path::Path, content: &str) -> String {
    std::fs::create_dir_all(path).unwrap();
    let run = |args: &[&str]| {
        let out = Cmd::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .unwrap_or_else(|e| panic!("git {:?} spawn failed: {e}", args));
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        out
    };
    run(&["init"]);
    run(&["config", "user.email", "t@t.com"]);
    run(&["config", "user.name", "T"]);
    std::fs::write(path.join("f"), content).unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "init"]);
    let out = run(&["rev-parse", "HEAD"]);
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn init_git_repo(path: &std::path::Path) -> String {
    init_git_repo_with_content(path, "content-a")
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn contains_human_readable_timestamp(log_line: &str) -> bool {
    fn is_supported_timestamp(value: &str) -> bool {
        chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").is_ok()
            || chrono::DateTime::parse_from_rfc3339(value).is_ok()
    }

    let has_bracketed_timestamp = log_line
        .strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .map(|(timestamp, _)| timestamp)
        .is_some_and(is_supported_timestamp);
    has_bracketed_timestamp
        || log_line.split_whitespace().any(|token| {
            let trimmed = token.trim_matches(|ch: char| matches!(ch, '[' | ']' | '(' | ')' | ','));
            let value = trimmed.strip_prefix("更新日時=").unwrap_or(trimmed);
            is_supported_timestamp(value)
        })
}

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
fn cargo_install_none_when_crates2_missing() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_missing_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_returns_none_and_logs_when_crates2_is_missing() {
    let tmp = unique_temp_dir("cargo_test_missing_log");

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs.iter().any(|msg| {
        msg.contains("リポジトリ=owner/myrepo")
            && msg.contains(".crates2.json")
            && msg.contains("cargo install メタデータファイルが見つからない")
    }));
}

#[test]
fn cargo_install_none_when_repo_not_in_crates2() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_notfound_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let json = make_crates2_json("other", "other-repo", "other-repo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok());
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_returns_none_and_logs_when_repo_not_in_crates2() {
    let tmp = unique_temp_dir("cargo_test_notfound_log");
    let json = make_crates2_json("other", "other-repo", "other-repo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs.iter().any(|msg| {
        msg.contains("リポジトリ=owner/myrepo")
            && msg.contains(".crates2.json")
            && msg.contains("cargo install メタデータ内に対象リポジトリが見つからない")
    }));
}

#[test]
fn cargo_install_matches_crates2_git_url_with_dot_git_suffix() {
    let tmp = unique_temp_dir("cargo_test_dot_git_match");
    let json = make_crates2_json_with_dot_git("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs.iter().any(|msg| {
        msg.contains("一致した cargo install エントリ=")
            && msg.contains("git+https://github.com/owner/myrepo.git#")
    }));
    assert!(!logs
        .iter()
        .any(|msg| msg.contains("cargo install メタデータ内に対象リポジトリが見つからない")));
}

#[test]
fn cargo_install_none_when_checkouts_dir_missing() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_nocheckouts_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_none_when_no_matching_checkout_dir() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_nomatch_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    std::fs::create_dir_all(checkouts.join("other-repo-abc123")).unwrap();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_prefix_does_not_match_longer_crate_name() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_prefix_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    std::fs::create_dir_all(checkouts.join("myrepo-extra-abc123")).unwrap();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
}

#[test]
fn cargo_install_none_when_multiple_checkout_dirs_match() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_multi_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    std::fs::create_dir_all(checkouts.join("myrepo-aaaa1111")).unwrap();
    std::fs::create_dir_all(checkouts.join("myrepo-bbbb2222")).unwrap();
    let mut logged = String::new();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |msg| {
            logged = msg.to_string();
        },
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
    assert!(logged.contains("checkout ディレクトリが複数見つかりました"));
}

#[test]
fn cargo_install_none_when_checkout_subdir_missing() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_nosubdir_{}", std::process::id()));
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(checkouts.join("myrepo-abc12345")).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();
    assert!(result.is_none());
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

    let (matches, inst, loc, remote) = result.expect("local clone failure must not block cgo");
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
