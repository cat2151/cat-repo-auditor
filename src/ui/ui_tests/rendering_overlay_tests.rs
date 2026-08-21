use super::*;
use unicode_width::UnicodeWidthStr;

#[test]
fn draw_ui_shows_workflow_repo_exist_overlay() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.open_workflow_repo_exist(vec![
        crate::github_local::WorkflowRepoExistCheck {
            workflow_file: String::from("call-a.yml"),
            installed_repos: vec![crate::github_local::WorkflowRepoExistRepo {
                name: String::from("repo-a"),
                updated_at: String::from("today"),
                updated_at_raw: String::from("2026-03-28T00:00:00Z"),
            }],
            missing_repos: vec![
                crate::github_local::WorkflowRepoExistRepo {
                    name: String::from("repo-b"),
                    updated_at: String::from("2d"),
                    updated_at_raw: String::from("2026-03-26T00:00:00Z"),
                },
                crate::github_local::WorkflowRepoExistRepo {
                    name: String::from("repo-c"),
                    updated_at: String::from("3w"),
                    updated_at_raw: String::from("2026-03-07T00:00:00Z"),
                },
            ],
        },
        crate::github_local::WorkflowRepoExistCheck {
            workflow_file: String::from("call-b.yml"),
            installed_repos: vec![],
            missing_repos: vec![crate::github_local::WorkflowRepoExistRepo {
                name: String::from("repo-a"),
                updated_at: String::from("today"),
                updated_at_raw: String::from("2026-03-28T00:00:00Z"),
            }],
        },
    ]);

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal).join("\n");

    assert!(rendered.contains("workflow repo exist check"));
    assert!(rendered.contains("call-a.yml"));
    assert!(rendered.contains("repo-a"));
    assert!(rendered.contains("repo-b"));
    assert!(rendered.contains("repo-c"));
    assert!(rendered.contains("today"));
    assert!(rendered.contains("2d"));
}

#[test]
fn draw_ui_shows_empty_workflow_repo_exist_overlay_message() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.open_workflow_repo_exist(vec![]);

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal).join("\n");

    assert!(rendered.contains("workflow repo exist check"));
    assert!(rendered.contains("no call-* workflows"));
    assert!(rendered.contains("(none)"));
}

#[test]
fn draw_ui_aligns_workflow_repo_column_with_wide_chars() {
    let backend = TestBackend::new(90, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.open_workflow_repo_exist(vec![crate::github_local::WorkflowRepoExistCheck {
        workflow_file: String::from("call-幅-test.yml"),
        installed_repos: vec![
            crate::github_local::WorkflowRepoExistRepo {
                name: String::from("repo-通常"),
                updated_at: String::from("today"),
                updated_at_raw: String::from("2026-03-28T00:00:00Z"),
            },
            crate::github_local::WorkflowRepoExistRepo {
                name: String::from("repo-とても長い名前です-甲乙丙丁戊己庚辛壬"),
                updated_at: String::from("2d"),
                updated_at_raw: String::from("2026-03-26T00:00:00Z"),
            },
        ],
        missing_repos: vec![],
    }]);

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let buffer = terminal.backend().buffer();
    let today_x = find_text_x(buffer, "today").unwrap() as usize;
    let two_days_x = find_text_x(buffer, "2d").unwrap() as usize;

    assert_eq!(
        today_x + UnicodeWidthStr::width("today"),
        two_days_x + UnicodeWidthStr::width("2d")
    );

    let rendered = rendered_lines(&terminal).join("\n");
    assert!(rendered.contains('…'));
}

#[test]
fn draw_ui_shows_cargo_ng_overlay_with_installed_and_remote_hashes() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.loading = false;
    app.repos[0].cargo_bin_check = Some(false);
    app.repos[0].cargo_installed_hash = String::from("4ecf42e931e9dc3af0dd89bd53351676a2899a23");
    app.repos[0].cargo_remote_hash = String::from("47049c0fe70d57e233d8943a4abab5bf780621bc");
    app.open_cargo_ng_from_repos();

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    // TestBackend は全角 1 文字を 2 セル分返すため、空白を落としてから比較する。
    let rendered = rendered_lines(&terminal).join("\n").replace(' ', "");
    assert!(
        rendered.contains("cargoinstallが古いアプリ"),
        "NG overlay title should be visible: {rendered}"
    );
    assert!(
        rendered.contains("focus-test"),
        "NG overlay should list the stale repo: {rendered}"
    );
    assert!(
        rendered.contains("4ecf42e9") && rendered.contains("47049c0f"),
        "NG overlay should show installed and remote hashes: {rendered}"
    );
}

#[test]
fn draw_ui_shows_cargo_ng_overlay_empty_state() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.loading = false;
    app.open_cargo_ng_from_repos();

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let rendered = rendered_lines(&terminal).join("\n").replace(' ', "");
    assert!(
        rendered.contains("cgoがNGのrepoはありません"),
        "empty NG overlay should say so instead of rendering a blank box: {rendered}"
    );
}
