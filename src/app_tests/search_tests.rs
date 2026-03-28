use super::support::*;
use super::*;

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
fn search_enter_resets_query_and_match_index() {
    let mut app = App::new(make_config());
    app.repos = vec![make_active_repo("alpha"), make_active_repo("beta")];
    app.rebuild_rows();
    app.search_query = String::from("stale");
    app.search_match_idx = 3;
    app.repo_move_down(1);
    let saved_cursor = app.row_cursor;

    app.search_enter();

    assert_eq!(app.search_state, SearchState::Active);
    assert!(app.search_query.is_empty());
    assert_eq!(app.search_match_idx, 0);
    assert_eq!(app.search_saved_cursor, saved_cursor);
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
    let first = app.selected_repo_idx();
    app.search_next_match();
    let second = app.selected_repo_idx();
    assert_ne!(first, second, "next match should move cursor");
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
    let first = app.selected_repo_idx();
    app.search_prev_match();
    let last = app.selected_repo_idx();
    assert_ne!(first, last, "prev match should move cursor");
}
