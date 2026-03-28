use super::support::*;
use super::*;

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
