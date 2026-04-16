use super::*;

#[test]
fn draw_ui_does_not_leak_hidden_background_progress_into_repo_name_when_columns_hidden() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.show_columns = false;
    app.repos[0].readme_ja = Some(true);
    app.repos[0].readme_ja_checked_at = String::from("2023-12-31T00:00:00Z");
    app.checking_repos.insert(String::from("focus-test"));

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        repo_line.contains("▶focus-test"),
        "repo name should stay plain when hidden columns have pending checks: {repo_line}"
    );
    assert!(
        SPINNER_FRAMES
            .iter()
            .all(|frame| !repo_line.contains(&format!("▶{frame}focus-test"))),
        "hidden-column progress should not be surfaced through the repo name: {repo_line}"
    );
}

#[test]
fn draw_ui_keeps_local_status_visible_while_non_local_checks_are_pending() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.repos[0].readme_ja = Some(true);
    app.repos[0].readme_ja_checked_at = String::from("2023-12-31T00:00:00Z");
    app.repos[0].local_status = crate::github::LocalStatus::Clean;
    app.checking_repos.insert(String::from("focus-test"));

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        SPINNER_FRAMES.iter().any(|frame| repo_line.contains(frame)),
        "visible cells should show spinners while pending: {repo_line}"
    );
    assert!(
        repo_line.contains("▶focus-test"),
        "repo name should no longer show an aggregate spinner: {repo_line}"
    );
    assert!(
        repo_line.contains("cle"),
        "local column should stay visible while only non-local checks are pending: {repo_line}"
    );
    assert!(
        !repo_line.contains('✔'),
        "stale cached checkmark should be hidden until the recheck completes: {repo_line}"
    );
}

#[test]
fn draw_ui_shows_local_spinner_only_while_local_refresh_is_pending() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.repos[0].local_status = crate::github::LocalStatus::Clean;
    app.pending_local_repos.insert(String::from("focus-test"));

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        SPINNER_FRAMES.iter().any(|frame| repo_line.contains(frame)),
        "local column should show pending spinner while local refresh is running: {repo_line}"
    );
    assert!(
        !repo_line.contains("cle"),
        "local column should hide stale status while local refresh is pending: {repo_line}"
    );
}

#[test]
fn draw_ui_shows_ja_wiki_workflow_spinners_while_startup_local_refresh_is_pending() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.repos[0].readme_ja_badge = Some(true);
    app.repos[0].readme_ja_badge_checked_at = String::from("local123");
    app.repos[0].deepwiki = Some(true);
    app.repos[0].deepwiki_checked_at = String::from("local123");
    app.repos[0].wf_workflows = Some(true);
    app.repos[0].wf_checked_at = String::from("local123");
    app.pending_local_repos.insert(String::from("focus-test"));

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        spinner_count(repo_line) >= 4,
        "local startup refresh should show spinners in local-sensitive columns: {repo_line}"
    );
    assert!(
        !repo_line.contains('✔'),
        "cached local-sensitive checkmarks should stay hidden while startup local refresh is pending: {repo_line}"
    );
}

#[test]
fn draw_ui_restores_local_status_text_after_repo_check_completes() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.loading = false;
    app.repos[0].local_status = crate::github::LocalStatus::Clean;

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        repo_line.contains("cle"),
        "local column should show the resolved local status after pending ends: {repo_line}"
    );
    assert!(
        SPINNER_FRAMES
            .iter()
            .all(|frame| !repo_line.contains(frame)),
        "local column should stop showing a spinner after pending ends: {repo_line}"
    );
}

#[test]
fn draw_ui_shows_pr_and_issue_pending_in_visible_columns_while_repo_is_pending() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.show_columns = false;
    app.issue_pr_pending_repos
        .insert(String::from("focus-test"));
    app.repos[0].open_prs = 7;
    app.repos[0].open_issues = 4;

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        SPINNER_FRAMES.iter().any(|frame| repo_line.contains(frame)),
        "visible PR/ISS columns should show pending while loading: {repo_line}"
    );
    assert!(
        !repo_line.contains("  7") && !repo_line.contains("  4"),
        "stale PR/ISS counts should be hidden while loading: {repo_line}"
    );
}

#[test]
fn draw_ui_shows_doc_and_pages_spinners_while_startup_repo_refresh_is_pending() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.repos[0].readme_ja = Some(true);
    app.repos[0].readme_ja_checked_at = String::from("2024-01-01T00:00:00Z");
    app.repos[0].pages = Some(true);
    app.repos[0].pages_checked_at = String::from("2024-01-01T00:00:00Z");
    app.issue_pr_pending_repos
        .insert(String::from("focus-test"));

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        spinner_count(repo_line) >= 4,
        "startup repo refresh should show spinners in PR/ISS/doc/pages columns: {repo_line}"
    );
    assert!(
        !repo_line.contains('✔'),
        "cached remote-sensitive checkmarks should stay hidden while startup repo refresh is pending: {repo_line}"
    );
}

#[test]
fn draw_ui_clears_pr_and_issue_spinner_per_repo_as_updates_arrive() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new(make_config());
    let mut alpha = make_repo("alpha");
    alpha.open_prs = 7;
    alpha.open_issues = 4;
    let mut beta = make_repo("beta");
    beta.open_prs = 2;
    beta.open_issues = 1;
    app.repos = vec![alpha, beta];
    app.show_columns = false;
    app.issue_pr_pending_repos.insert(String::from("alpha"));
    app.rebuild_rows();

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let alpha_line = rendered
        .iter()
        .find(|line| line.contains("▶alpha"))
        .map(String::as_str)
        .expect("repo list should contain alpha row");
    let beta_line = rendered
        .iter()
        .find(|line| line.contains(" beta") && !line.contains("│ PR:"))
        .map(String::as_str)
        .expect("repo list should contain beta row");

    assert!(
        SPINNER_FRAMES
            .iter()
            .any(|frame| alpha_line.contains(frame)),
        "pending repo should keep spinner until its issue/pr update arrives: {alpha_line}"
    );
    assert!(
        !alpha_line.contains("  7") && !alpha_line.contains("  4"),
        "pending repo should hide stale counts: {alpha_line}"
    );
    assert!(
        !SPINNER_FRAMES.iter().any(|frame| beta_line.contains(frame)),
        "completed repo should not keep issue/pr spinner: {beta_line}"
    );
    assert!(
        beta_line.contains("  2") && beta_line.contains("  1"),
        "completed repo should already show resolved counts: {beta_line}"
    );
}

#[test]
fn draw_ui_shows_cgo_spinner_while_post_update_polling_is_active() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.loading = false;
    app.repos[0].cargo_install = Some(false);
    app.repos[0].cargo_installed_hash = String::from("installed-old");
    app.repos[0].cargo_remote_hash = String::from("remote-new");
    app.start_auto_update_cargo_hash_polling("focus-test");

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        SPINNER_FRAMES.iter().any(|frame| repo_line.contains(frame)),
        "cgo column should show a spinner while cargo hash polling is active: {repo_line}"
    );
    assert!(
        !repo_line.contains("old"),
        "stale cargo status should be hidden while polling is active: {repo_line}"
    );
}

#[test]
fn draw_ui_shows_cgo_spinner_while_startup_cargo_check_is_pending() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.loading = false;
    app.repos[0].local_head_hash = String::from("local123");
    app.repos[0].cargo_checked_at = String::from("local123");
    app.repos[0].updated_at_raw = String::from("2024-01-01T00:00:00Z");
    app.repos[0].cargo_remote_hash_checked_at = String::from("2024-01-01T00:00:00Z");
    app.repos[0].cargo_installed_hash = String::from("same-hash");
    app.repos[0].cargo_remote_hash = String::from("same-hash");
    app.pending_cargo_repos.insert(String::from("focus-test"));

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        SPINNER_FRAMES.iter().any(|frame| repo_line.contains(frame)),
        "cgo column should show a spinner while startup cargo check is pending: {repo_line}"
    );
    assert!(
        !repo_line.contains("ok"),
        "cached cargo status should stay hidden until the current cargo check finishes: {repo_line}"
    );
}

#[test]
fn draw_ui_shows_cgo_old_from_hash_mismatch_even_if_cached_flag_is_ok() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.loading = false;
    app.repos[0].cargo_install = Some(true);
    app.repos[0].cargo_installed_hash = String::from("installed-old");
    app.repos[0].cargo_remote_hash = String::from("remote-new");

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        repo_line.contains("old"),
        "cgo column should show old when installed and remote hashes differ: {repo_line}"
    );
    assert!(
        !repo_line.contains(" ok"),
        "cgo column should not rely on cached cargo_install=true when hashes differ: {repo_line}"
    );
}

#[test]
fn draw_ui_shows_cgo_ok_from_hash_match_even_if_cached_flag_is_old() {
    let backend = TestBackend::new(120, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.loading = false;
    app.repos[0].cargo_install = Some(false);
    app.repos[0].cargo_installed_hash = String::from("same-hash");
    app.repos[0].cargo_remote_hash = String::from("same-hash");

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal);
    let repo_line = rendered
        .iter()
        .find(|line| line.contains('▶') && line.contains("focus-test"))
        .map(String::as_str)
        .expect("repo list should contain selected repo row");
    assert!(
        repo_line.contains("ok"),
        "cgo column should show ok when installed and remote hashes match: {repo_line}"
    );
    assert!(
        !repo_line.contains("old"),
        "cgo column should not rely on cached cargo_install=false when hashes match: {repo_line}"
    );
}
