use super::*;
use crate::{
    config::Config,
    github::{IssueOrPr, LocalStatus, RepoInfo},
    ui::{Focus, RepoRow, SearchState},
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn make_config() -> Config {
    Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: None,
        auto_pull: false,
    }
}

fn make_repo(name: &str) -> RepoInfo {
    RepoInfo {
        name: name.to_string(),
        full_name: format!("owner/{name}"),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        open_issues: 0,
        open_prs: 0,
        is_private: false,
        local_status: LocalStatus::Clean,
        has_local_git: true,
        staging_files: vec![],
        issues: vec![],
        prs: vec![],
        readme_ja: None,
        readme_ja_checked_at: String::new(),
        readme_ja_badge: None,
        readme_ja_badge_checked_at: String::new(),
        pages: None,
        pages_checked_at: String::new(),
        deepwiki: None,
        deepwiki_checked_at: String::new(),
        cargo_install: None,
        cargo_checked_at: String::new(),
        cargo_remote_hash: String::new(),
        cargo_installed_hash: String::new(),
        wf_workflows: None,
        wf_checked_at: String::new(),
    }
}

fn make_issue(number: u64, title: &str, repo_full: &str) -> IssueOrPr {
    IssueOrPr {
        title: title.to_string(),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        number,
        repo_full: repo_full.to_string(),
        is_pr: false,
        closes_issue: None,
    }
}

fn make_pr(number: u64, title: &str, repo_full: &str, closes: Option<u64>) -> IssueOrPr {
    IssueOrPr {
        title: title.to_string(),
        updated_at: String::from("2024-01-01"),
        updated_at_raw: String::from("2024-01-01T00:00:00Z"),
        number,
        repo_full: repo_full.to_string(),
        is_pr: true,
        closes_issue: closes,
    }
}

fn make_active_repo(name: &str) -> RepoInfo {
    let mut r = make_repo(name);
    r.open_prs = 1; // group 0: no separator, cursor starts at 0
    r
}

fn repo_count(app: &App) -> usize {
    app.filtered_rows
        .iter()
        .filter(|r| matches!(r, RepoRow::Repo(_)))
        .count()
}

// ── num_prefix ───────────────────────────────────────────────────────────────

#[test]
fn push_digit_builds_number() {
    let mut app = App::new(make_config());
    app.push_digit(1);
    app.push_digit(2);
    app.push_digit(3);
    assert_eq!(app.num_prefix, 123);
}

#[test]
fn consume_prefix_returns_one_when_zero() {
    let mut app = App::new(make_config());
    assert_eq!(app.consume_prefix(), 1);
    assert_eq!(app.num_prefix, 0);
}

#[test]
fn consume_prefix_returns_value_and_resets() {
    let mut app = App::new(make_config());
    app.push_digit(5);
    assert_eq!(app.consume_prefix(), 5);
    assert_eq!(app.num_prefix, 0);
}

// ── repo navigation ──────────────────────────────────────────────────────────

#[test]
fn repo_move_down_advances_cursor() {
    let mut app = App::new(make_config());
    app.repos = vec![
        make_active_repo("a"),
        make_active_repo("b"),
        make_active_repo("c"),
    ];
    app.rebuild_rows();
    assert_eq!(app.row_cursor, 0);
    app.repo_move_down(1);
    assert_eq!(app.row_cursor, 1);
    app.repo_move_down(1);
    assert_eq!(app.row_cursor, 2);
}

#[test]
fn repo_move_down_stops_at_last() {
    let mut app = App::new(make_config());
    app.repos = vec![make_active_repo("a"), make_active_repo("b")];
    app.rebuild_rows();
    app.repo_move_down(10);
    assert_eq!(app.row_cursor, 1);
}

#[test]
fn repo_move_up_decrements_cursor() {
    let mut app = App::new(make_config());
    app.repos = vec![
        make_active_repo("a"),
        make_active_repo("b"),
        make_active_repo("c"),
    ];
    app.rebuild_rows();
    app.repo_move_down(2);
    assert_eq!(app.row_cursor, 2);
    app.repo_move_up(1);
    assert_eq!(app.row_cursor, 1);
}

#[test]
fn repo_move_up_stays_at_top() {
    let mut app = App::new(make_config());
    app.repos = vec![make_active_repo("a"), make_active_repo("b")];
    app.rebuild_rows();
    app.repo_move_up(10);
    assert_eq!(app.row_cursor, 0);
}

#[test]
fn selected_repo_idx_returns_correct_index() {
    let mut app = App::new(make_config());
    app.repos = vec![
        make_active_repo("a"),
        make_active_repo("b"),
        make_active_repo("c"),
    ];
    app.rebuild_rows();
    assert_eq!(app.selected_repo_idx(), Some(0));
    app.repo_move_down(1);
    assert_eq!(app.selected_repo_idx(), Some(1));
    app.repo_move_down(1);
    assert_eq!(app.selected_repo_idx(), Some(2));
}

#[test]
fn selected_repo_returns_repo_info() {
    let mut app = App::new(make_config());
    app.repos = vec![make_active_repo("alpha"), make_active_repo("beta")];
    app.rebuild_rows();
    assert_eq!(app.selected_repo().unwrap().name, "alpha");
    app.repo_move_down(1);
    assert_eq!(app.selected_repo().unwrap().name, "beta");
}

// ── filter / search ──────────────────────────────────────────────────────────

#[test]
fn apply_filter_empty_query_shows_all() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("alpha"), make_repo("beta")];
    app.rebuild_rows();
    assert_eq!(repo_count(&app), 2);
}

#[test]
fn apply_filter_with_query_filters_by_name() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("alpha"), make_repo("beta"), make_repo("alphabet")];
    app.rebuild_rows();
    app.search_query = String::from("alpha");
    app.apply_filter();
    let names: Vec<&str> = app
        .filtered_rows
        .iter()
        .filter_map(|r| {
            if let RepoRow::Repo(i) = r {
                Some(app.repos[*i].name.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(names.contains(&"alpha"), "alpha expected");
    assert!(names.contains(&"alphabet"), "alphabet expected");
    assert!(!names.contains(&"beta"), "beta should be filtered out");
}

#[test]
fn apply_filter_multi_term_and_logic() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("foo-bar"), make_repo("foo-baz"), make_repo("qux")];
    app.rebuild_rows();
    app.search_query = String::from("foo bar");
    app.apply_filter();
    let names: Vec<&str> = app
        .filtered_rows
        .iter()
        .filter_map(|r| {
            if let RepoRow::Repo(i) = r {
                Some(app.repos[*i].name.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(names.contains(&"foo-bar"), "foo-bar expected");
    assert!(
        !names.contains(&"foo-baz"),
        "foo-baz should not match 'bar'"
    );
    assert!(!names.contains(&"qux"), "qux should not match");
}

#[test]
fn search_push_filters_and_jumps_to_first_match() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("alpha"), make_repo("beta")];
    app.rebuild_rows();
    app.search_enter();
    app.search_push('b');
    assert_eq!(repo_count(&app), 1);
    assert_eq!(app.selected_repo_idx(), Some(1));
}

#[test]
fn search_pop_expands_filter() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("beta"), make_repo("beta2")];
    app.rebuild_rows();
    app.search_enter();
    for c in "beta2".chars() {
        app.search_push(c);
    }
    assert_eq!(repo_count(&app), 1);
    app.search_pop();
    assert_eq!(repo_count(&app), 2);
}

#[test]
fn search_confirm_clears_filter_and_preserves_selection() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("alpha"), make_repo("beta"), make_repo("gamma")];
    app.rebuild_rows();
    app.search_query = String::from("beta");
    app.apply_filter();
    assert_eq!(app.selected_repo_idx(), Some(1));
    app.search_confirm();
    assert!(app.search_query.is_empty(), "query should be cleared");
    assert_eq!(app.search_state, SearchState::Off);
    assert_eq!(
        app.selected_repo_idx(),
        Some(1),
        "selection should remain on beta"
    );
}

#[test]
fn search_cancel_restores_cursor_position() {
    let mut app = App::new(make_config());
    app.repos = vec![make_repo("alpha"), make_repo("beta"), make_repo("gamma")];
    app.rebuild_rows();
    app.repo_move_down(2);
    assert_eq!(app.selected_repo_idx(), Some(2));
    app.search_enter();
    app.search_push('b');
    assert_eq!(app.selected_repo_idx(), Some(1));
    app.search_cancel();
    assert_eq!(app.search_state, SearchState::Off);
    assert_eq!(
        app.selected_repo_idx(),
        Some(2),
        "cursor should be restored to gamma"
    );
}

#[test]
fn search_next_match_cycles_forward() {
    let mut app = App::new(make_config());
    // Use active repos (open_issues=1) so group 0, no separator before them
    app.repos = vec![
        make_active_repo("foo1"),
        make_active_repo("bar"),
        make_active_repo("foo2"),
    ];
    app.rebuild_rows();
    // Use search_enter + search_push so cursor is reset to 0 (first match)
    app.search_enter();
    for c in "foo".chars() {
        app.search_push(c);
    }
    // filtered_rows: [Repo(0)=foo1, Repo(2)=foo2], cursor at 0 → selected = Some(0)
    let first = app.selected_repo_idx();
    app.search_next_match();
    let second = app.selected_repo_idx();
    assert_ne!(first, second, "next match should move cursor");
    // next again wraps around
    app.search_next_match();
    assert_eq!(
        app.selected_repo_idx(),
        first,
        "should wrap around to first match"
    );
}

#[test]
fn search_prev_match_cycles_backward() {
    let mut app = App::new(make_config());
    app.repos = vec![
        make_active_repo("foo1"),
        make_active_repo("bar"),
        make_active_repo("foo2"),
    ];
    app.rebuild_rows();
    app.search_enter();
    for c in "foo".chars() {
        app.search_push(c);
    }
    // cursor at 0 = foo1 (first match)
    let first = app.selected_repo_idx();
    // prev from first should wrap to last
    app.search_prev_match();
    let last = app.selected_repo_idx();
    assert_ne!(first, last, "prev match should move cursor");
}

// ── detail pane navigation ───────────────────────────────────────────────────

#[test]
fn detail_move_down_and_up() {
    let mut app = App::new(make_config());
    let mut repo = make_repo("a");
    repo.open_issues = 2;
    repo.issues = vec![
        make_issue(1, "issue1", "owner/a"),
        make_issue(2, "issue2", "owner/a"),
    ];
    app.repos = vec![repo];
    app.rebuild_rows();
    assert_eq!(app.detail_selected, 0);
    app.detail_move_down(1);
    assert_eq!(app.detail_selected, 1);
    app.detail_move_up(1);
    assert_eq!(app.detail_selected, 0);
}

#[test]
fn detail_move_down_stops_at_last() {
    let mut app = App::new(make_config());
    let mut repo = make_repo("a");
    repo.open_issues = 2;
    repo.issues = vec![
        make_issue(1, "i1", "owner/a"),
        make_issue(2, "i2", "owner/a"),
    ];
    app.repos = vec![repo];
    app.rebuild_rows();
    app.detail_move_down(100);
    assert_eq!(app.detail_selected, 1);
}

#[test]
fn detail_move_up_stays_at_zero() {
    let mut app = App::new(make_config());
    let mut repo = make_repo("a");
    repo.issues = vec![make_issue(1, "i1", "owner/a")];
    app.repos = vec![repo];
    app.rebuild_rows();
    app.detail_move_up(10);
    assert_eq!(app.detail_selected, 0);
}

#[test]
fn selected_detail_url_returns_issue_url() {
    let mut app = App::new(make_config());
    let mut repo = make_repo("a");
    repo.issues = vec![make_issue(42, "title", "owner/a")];
    app.repos = vec![repo];
    app.rebuild_rows();
    let url = app.selected_detail_url().unwrap();
    assert_eq!(url, "https://github.com/owner/a/issues/42");
}

#[test]
fn selected_detail_url_returns_pr_url() {
    let mut app = App::new(make_config());
    let mut repo = make_repo("a");
    repo.prs = vec![make_pr(7, "fix", "owner/a", None)];
    app.repos = vec![repo];
    app.rebuild_rows();
    let url = app.selected_detail_url().unwrap();
    assert_eq!(url, "https://github.com/owner/a/pull/7");
}

// ── scroll adjustment ─────────────────────────────────────────────────────────

#[test]
fn adjust_row_scroll_scrolls_down_when_cursor_out_of_view() {
    let mut app = App::new(make_config());
    app.repos = (0..10).map(|i| make_repo(&format!("r{i}"))).collect();
    app.rebuild_rows();
    app.row_cursor = 7;
    app.row_scroll = 0;
    app.adjust_row_scroll(5);
    // cursor 7, visible 5 → scroll should be 7 + 1 - 5 = 3
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
    // PR appears after issue in detail items; verify url is a pull URL
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
