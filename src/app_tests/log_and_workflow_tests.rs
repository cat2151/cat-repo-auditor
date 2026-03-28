use super::support::*;
use super::*;

#[test]
fn adjust_row_scroll_scrolls_down_when_cursor_out_of_view() {
    let mut app = App::new(make_config());
    app.repos = (0..10).map(|i| make_repo(&format!("r{i}"))).collect();
    app.rebuild_rows();
    app.row_cursor = 7;
    app.row_scroll = 0;
    app.adjust_row_scroll(5);
    assert_eq!(app.row_scroll, 3);
}

#[test]
fn adjust_row_scroll_scrolls_up_when_cursor_above_view() {
    let mut app = App::new(make_config());
    app.repos = (0..10).map(|i| make_repo(&format!("r{i}"))).collect();
    app.rebuild_rows();
    app.row_cursor = 2;
    app.row_scroll = 5;
    app.adjust_row_scroll(5);
    assert_eq!(app.row_scroll, 2);
}

#[test]
fn focus_detail_first_pr_or_issue_jumps_to_first_pr() {
    let mut app = App::new(make_config());
    let mut repo = make_repo("a");
    repo.issues = vec![make_issue(1, "issue", "owner/a")];
    repo.prs = vec![make_pr(2, "pr", "owner/a", None)];
    app.repos = vec![repo];
    app.rebuild_rows();
    app.focus_detail_first_pr_or_issue();
    assert_eq!(app.focus, Focus::Detail);
    let url = app.selected_detail_url().unwrap();
    assert!(url.contains("/pull/"), "should point to PR, got: {url}");
}

#[test]
fn toggle_log_switches_visibility() {
    let mut app = App::new(make_config());
    assert!(!app.show_log);
    app.toggle_log();
    assert!(app.show_log);
    app.toggle_log();
    assert!(!app.show_log);
}

#[test]
fn append_log_line_adds_line() {
    let mut app = App::new(make_config());
    app.append_log_line(String::from("line1"));
    app.append_log_line(String::from("line2"));
    assert_eq!(app.log_lines, vec!["line1", "line2"]);
}

#[test]
fn append_log_line_caps_history() {
    let mut app = App::new(make_config());
    for i in 0..2_100 {
        app.append_log_line(format!("line{i}"));
    }
    assert_eq!(app.log_lines.len(), 2_000);
    assert_eq!(app.log_lines.first().unwrap(), "line100");
    assert_eq!(app.log_lines.last().unwrap(), "line2099");
}

#[test]
fn set_log_lines_caps_history() {
    let mut app = App::new(make_config());
    let lines: Vec<String> = (0..2_100).map(|i| format!("line{i}")).collect();
    app.set_log_lines(lines);
    assert_eq!(app.log_lines.len(), 2_000);
    assert_eq!(app.log_lines.first().unwrap(), "line100");
    assert_eq!(app.log_lines.last().unwrap(), "line2099");
}

#[test]
fn workflow_repo_exist_open_and_select() {
    let mut app = App::new(make_config());
    let items = vec![
        crate::github_local::WorkflowRepoExistCheck {
            workflow_file: String::from("call-a.yml"),
            installed_repos: vec![crate::github_local::WorkflowRepoExistRepo {
                name: String::from("repo-a"),
                updated_at: String::from("today"),
                updated_at_raw: String::from("2026-03-28T00:00:00Z"),
            }],
            missing_repos: vec![crate::github_local::WorkflowRepoExistRepo {
                name: String::from("repo-b"),
                updated_at: String::from("2d"),
                updated_at_raw: String::from("2026-03-26T00:00:00Z"),
            }],
        },
        crate::github_local::WorkflowRepoExistCheck {
            workflow_file: String::from("call-b.yml"),
            installed_repos: vec![crate::github_local::WorkflowRepoExistRepo {
                name: String::from("repo-b"),
                updated_at: String::from("2d"),
                updated_at_raw: String::from("2026-03-26T00:00:00Z"),
            }],
            missing_repos: vec![crate::github_local::WorkflowRepoExistRepo {
                name: String::from("repo-a"),
                updated_at: String::from("today"),
                updated_at_raw: String::from("2026-03-28T00:00:00Z"),
            }],
        },
    ];

    app.open_workflow_repo_exist(items);
    app.workflow_repo_exist_move_down(1);

    assert!(app.show_workflow_repo_exist);
    assert_eq!(
        app.selected_workflow_repo_exist().unwrap().workflow_file,
        "call-b.yml"
    );
}

#[test]
fn adjust_workflow_repo_exist_scroll_tracks_selection() {
    let mut app = App::new(make_config());
    app.open_workflow_repo_exist(
        (0..4)
            .map(|i| crate::github_local::WorkflowRepoExistCheck {
                workflow_file: format!("call-{i}.yml"),
                installed_repos: vec![],
                missing_repos: vec![],
            })
            .collect(),
    );

    app.workflow_repo_exist_move_down(3);
    app.adjust_workflow_repo_exist_scroll(2);

    assert_eq!(app.workflow_repo_exist_scroll, 2);
}
