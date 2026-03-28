use super::support::*;
use super::*;

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
