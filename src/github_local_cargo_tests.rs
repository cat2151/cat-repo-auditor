use super::{check_cargo_git_install_inner, get_cargo_bins_inner};
use std::process::Command as Cmd;
use std::time::Duration;

fn make_crates2_json(owner: &str, repo: &str, crate_name: &str) -> String {
    let key = format!(
        "{crate_name} 0.1.0 (git+https://github.com/{owner}/{repo}#0123456789abcdef0123456789abcdef01234567)"
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
    assert!(logged.contains("multiple checkouts"));
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
fn cargo_install_returns_some_true_when_hashes_match() {
    let tmp = std::env::temp_dir().join(format!("cargo_test_match_{}", std::process::id()));
    let local_repo = tmp.join("repos").join("myrepo");
    let local_hash = init_git_repo(&local_repo);

    let cargo_home = tmp.join("cargo_home");
    let installed_sub = cargo_home
        .join("git")
        .join("checkouts")
        .join("myrepo-xyz99999")
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

    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc) = result.expect("should return Some");
    assert!(matches, "hashes should match: inst={inst} loc={loc}");
    assert_eq!(inst, local_hash);
    assert_eq!(loc, local_hash);
}

#[test]
fn cargo_install_logs_hash_source_details() {
    let tmp = unique_temp_dir("cargo_test_hash_log");
    let local_repo_path = tmp.join("repos").join("myrepo");
    init_git_repo(&local_repo_path);

    let cargo_home = tmp.join("cargo_home");
    let installed_checkout_path = cargo_home
        .join("git")
        .join("checkouts")
        .join("myrepo-xyz99999")
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

    let mut logs = Vec::new();
    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        |msg| logs.push(msg.to_string()),
    );
    std::fs::remove_dir_all(&tmp).ok();

    let crates2_path_display = crates2_path.display().to_string();
    let installed_checkout_display = installed_checkout_path.display().to_string();
    assert!(result.is_some());
    assert!(logs.iter().any(|msg| msg.contains(&crates2_path_display)));
    assert!(logs
        .iter()
        .any(|msg| msg.contains(&installed_checkout_display)));
    assert!(logs
        .iter()
        .any(|msg| msg.contains("git -C") && msg.contains("rev-parse HEAD")));
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

    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (matches, inst, loc) = result.expect("should return Some");
    assert!(!matches, "hashes should differ: inst={inst} loc={loc}");
    assert_eq!(inst, installed_hash);
    assert_eq!(loc, local_hash);
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

    let result = check_cargo_git_install_inner(
        "owner",
        "myrepo",
        tmp.join("repos").to_str().unwrap(),
        cargo_home.to_str().unwrap(),
        |_| {},
    );
    std::fs::remove_dir_all(&tmp).ok();

    let (_matches, inst, _loc) = result.expect("should return Some");
    assert_eq!(inst, expected_installed_hash);
    assert_ne!(inst, local_hash);
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
fn get_cargo_bins_returns_none_when_repo_is_not_installed() {
    let tmp = unique_temp_dir("cargo_test_bins_missing");
    let json = make_crates2_json("owner", "other-repo", "catrepo");
    std::fs::write(tmp.join(".crates2.json"), json).unwrap();

    let bins = get_cargo_bins_inner(&tmp, "owner", "myrepo");

    std::fs::remove_dir_all(&tmp).ok();
    assert_eq!(bins, None);
}
