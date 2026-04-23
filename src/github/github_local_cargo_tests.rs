use super::{
    append_cargo_check_after_auto_update_log_for_path, check_cargo_git_install_inner,
    check_cargo_git_install_inner_with_remote_hash,
    check_cargo_git_install_status_with_remote_failure_and_logger,
    check_cargo_git_install_with_remote_hash_and_logger, get_cargo_bins_inner,
    CargoGitInstallCheck,
};
use std::process::Command as Cmd;
use std::time::Duration;

#[path = "github_local_cargo_tests_additional.rs"]
mod additional_tests;
#[path = "github_local_cargo_tests_success.rs"]
mod success_tests;

const DEFAULT_METADATA_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

fn make_crates2_json(owner: &str, repo: &str, crate_name: &str) -> String {
    make_crates2_json_with_revision(owner, repo, crate_name, DEFAULT_METADATA_REVISION)
}

fn make_crates2_json_with_revision(
    owner: &str,
    repo: &str,
    crate_name: &str,
    revision: &str,
) -> String {
    make_crates2_json_with_repo_url_and_revision(
        owner,
        repo,
        crate_name,
        &format!("{repo}#"),
        revision,
    )
}

fn make_crates2_json_with_dot_git(owner: &str, repo: &str, crate_name: &str) -> String {
    make_crates2_json_with_repo_url_and_revision(
        owner,
        repo,
        crate_name,
        &format!("{repo}.git#"),
        DEFAULT_METADATA_REVISION,
    )
}

fn make_crates2_json_with_repo_url_and_revision(
    owner: &str,
    _repo: &str,
    crate_name: &str,
    repo_url_suffix: &str,
    revision: &str,
) -> String {
    let key =
        format!("{crate_name} 0.1.0 (git+https://github.com/{owner}/{repo_url_suffix}{revision})");
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
fn cargo_install_prefers_repo_checkout_name_when_crate_name_differs() {
    let tmp = unique_temp_dir("cargo_test_repo_checkout_name");
    let cargo_home = tmp.join("cargo_home");
    let checkout_base = cargo_home
        .join("git")
        .join("checkouts")
        .join("clap-mml-play-server-abc12345");
    let old_checkout = checkout_base.join("f786784");
    let old_metadata_revision = init_git_repo_with_content(&old_checkout, "old-installed-content");
    std::thread::sleep(Duration::from_millis(1_100));
    let latest_checkout = checkout_base.join("dcc5fe5");
    let installed_hash = init_git_repo_with_content(&latest_checkout, "latest-installed-content");
    assert_ne!(old_metadata_revision, installed_hash);
    let json = make_crates2_json_with_revision(
        "cat2151",
        "clap-mml-play-server",
        "clap-mml-render-server",
        &old_metadata_revision,
    );
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner_with_remote_hash(
        "cat2151",
        "clap-mml-play-server",
        "/nonexistent",
        cargo_home.to_str().unwrap(),
        &installed_hash,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc, remote) = result.expect("repo-name checkout should be found");
    assert!(matches);
    assert_eq!(inst, installed_hash);
    assert_eq!(loc, "");
    assert_eq!(remote, installed_hash);
    assert!(logs.iter().any(|msg| {
        msg.contains(
            "checkout dir 名の探索候補=[\"clap-mml-play-server\", \"clap-mml-render-server\"]",
        )
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("hash 取得候補 dir 名一覧=[\"clap-mml-play-server-abc12345\"]")
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("更新日時順の checkout subdir 候補[0]=")
            && msg.contains("dcc5fe5")
            && msg.contains("更新日時=")
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("参考: metadata revision と checkout HEAD が一致しません")
            && msg.contains(&old_metadata_revision)
            && msg.contains(&installed_hash)
    }));
}

#[test]
fn cargo_install_uses_repo_checkout_name_for_cat_self_update_app() {
    let tmp = unique_temp_dir("cargo_test_cat_self_update_repo_checkout");
    let cargo_home = tmp.join("cargo_home");
    let checkout_subdir = cargo_home
        .join("git")
        .join("checkouts")
        .join("cat-self-update-abc12345")
        .join("head1234");
    let installed_hash = init_git_repo_with_content(&checkout_subdir, "installed-content");
    let json = make_crates2_json_with_revision(
        "cat2151",
        "cat-self-update",
        "cat-self-update-app",
        DEFAULT_METADATA_REVISION,
    );
    std::fs::write(cargo_home.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner_with_remote_hash(
        "cat2151",
        "cat-self-update",
        "/nonexistent",
        cargo_home.to_str().unwrap(),
        &installed_hash,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc, remote) =
        result.expect("repo-name checkout should be enough when crate name differs");
    assert!(matches);
    assert_eq!(inst, installed_hash);
    assert_eq!(loc, "");
    assert_eq!(remote, installed_hash);
    assert!(logs.iter().any(|msg| {
        msg.contains("一致した crate 名=\"cat-self-update-app\"")
            && msg.contains("一致した git repo 名=\"cat-self-update\"")
            && msg.contains("metadata revision source=.crates2.json")
    }));
    assert!(logs.iter().any(|msg| {
        msg.contains("hash 取得候補 dir 名一覧=[\"cat-self-update-abc12345\"]")
    }));
}

#[test]
fn cargo_install_failed_when_remote_hash_missing() {
    let tmp = unique_temp_dir("cargo_test_remote_missing");
    let checkout_subdir = tmp
        .join("git")
        .join("checkouts")
        .join("myrepo-abc12345")
        .join("head1234");
    let installed_hash = init_git_repo_with_content(&checkout_subdir, "installed-content");
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_status_with_remote_failure_and_logger(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert_eq!(result, CargoGitInstallCheck::Failed);
    assert!(logs.iter().any(|msg| msg.contains(&format!(
        "インストール済み checkout のコミットハッシュを取得しました: {installed_hash}"
    ))));
    assert!(logs
        .iter()
        .any(|msg| msg.contains("remote のコミットハッシュ取得に失敗しました")));
}

#[test]
fn cargo_install_none_when_checkouts_dir_missing() {
    let tmp = unique_temp_dir("cargo_test_nocheckouts");
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner_with_remote_hash(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        DEFAULT_METADATA_REVISION,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs.iter().any(|msg| {
        msg.contains("cargo checkouts ディレクトリの読み取りに失敗しました")
            && msg.contains("git")
            && msg.contains("checkouts")
    }));
}

#[test]
fn cargo_install_none_when_no_matching_checkout_dir() {
    let tmp = unique_temp_dir("cargo_test_nomatch");
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    std::fs::create_dir_all(checkouts.join("other-repo-abc123")).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner_with_remote_hash(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        DEFAULT_METADATA_REVISION,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs
        .iter()
        .any(|msg| msg.contains("に対応する checkout ディレクトリが見つかりません")));
}

#[test]
fn cargo_install_none_when_only_longer_checkout_dir_name_exists() {
    let tmp = unique_temp_dir("cargo_test_prefix");
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    std::fs::create_dir_all(checkouts.join("myrepo-extra-abc123")).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner_with_remote_hash(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        DEFAULT_METADATA_REVISION,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs
        .iter()
        .any(|msg| msg.contains("に対応する checkout ディレクトリが見つかりません")));
    assert!(!logs
        .iter()
        .any(|msg| msg.contains("hash 取得候補 dir 名一覧=[\"myrepo-extra-abc123\"]")));
}

#[test]
fn cargo_install_none_when_multiple_checkout_dirs_match() {
    let tmp = unique_temp_dir("cargo_test_multi");
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(&checkouts).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();
    std::fs::create_dir_all(checkouts.join("myrepo-aaaa1111")).unwrap();
    std::fs::create_dir_all(checkouts.join("myrepo-bbbb2222")).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner_with_remote_hash(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        DEFAULT_METADATA_REVISION,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs
        .iter()
        .any(|msg| msg.contains("checkout ディレクトリが複数見つかりました")));
}

#[test]
fn cargo_install_none_when_checkout_subdir_missing() {
    let tmp = unique_temp_dir("cargo_test_nosubdir");
    let checkouts = tmp.join("git").join("checkouts");
    std::fs::create_dir_all(checkouts.join("myrepo-abc12345")).unwrap();
    let json = make_crates2_json("owner", "myrepo", "myrepo");
    std::fs::write(tmp.join(".crates2.json"), &json).unwrap();

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner_with_remote_hash(
        "owner",
        "myrepo",
        "/nonexistent",
        tmp.to_str().unwrap(),
        DEFAULT_METADATA_REVISION,
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    assert!(result.is_none());
    assert!(logs
        .iter()
        .any(|msg| msg.contains("checkout ディレクトリに候補となる subdir がありません")));
}
