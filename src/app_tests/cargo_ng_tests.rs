use super::support::{make_config, make_repo};
use super::*;
use crate::app::cargo_ng::collect_cargo_ng_items;

fn app_with(repos: Vec<RepoInfo>) -> App {
    let mut app = App::new(make_config());
    app.repos = repos;
    app.rebuild_rows();
    app
}

fn ng_repo(name: &str) -> RepoInfo {
    let mut repo = make_repo(name);
    repo.cargo_bin_check = Some(false);
    repo.cargo_installed_hash = String::from("4ecf42e931e9dc3af0dd89bd53351676a2899a23");
    repo.cargo_remote_hash = String::from("47049c0fe70d57e233d8943a4abab5bf780621bc");
    repo
}

fn ok_repo(name: &str) -> RepoInfo {
    let mut repo = make_repo(name);
    repo.cargo_bin_check = Some(true);
    repo.cargo_installed_hash = String::from("47049c0fe70d57e233d8943a4abab5bf780621bc");
    repo.cargo_remote_hash = String::from("47049c0fe70d57e233d8943a4abab5bf780621bc");
    repo
}

#[test]
fn collect_cargo_ng_items_picks_only_binary_reported_stale_repos() {
    let repos = vec![ok_repo("ok-repo"), ng_repo("ng-repo"), make_repo("plain")];
    let items = collect_cargo_ng_items(&repos);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "ng-repo");
    assert_eq!(
        items[0].installed_hash,
        "4ecf42e931e9dc3af0dd89bd53351676a2899a23"
    );
    assert_eq!(
        items[0].remote_hash,
        "47049c0fe70d57e233d8943a4abab5bf780621bc"
    );
}

#[test]
fn auto_open_cargo_ng_once_opens_only_the_first_time() {
    let mut app = app_with(vec![ng_repo("ng-repo")]);
    assert!(app.auto_open_cargo_ng_once());
    assert!(app.show_cargo_ng);

    app.close_cargo_ng();
    assert!(!app.auto_open_cargo_ng_once());
    assert!(!app.show_cargo_ng);
}

#[test]
fn auto_open_cargo_ng_once_opens_again_after_reset() {
    let mut app = app_with(vec![ng_repo("ng-repo")]);
    assert!(app.auto_open_cargo_ng_once());
    app.close_cargo_ng();
    app.reset_cargo_ng_auto_shown();
    assert!(app.auto_open_cargo_ng_once());
}

#[test]
fn auto_open_cargo_ng_once_does_nothing_without_ng_repos() {
    let mut app = app_with(vec![ok_repo("ok-repo")]);
    assert!(!app.auto_open_cargo_ng_once());
    assert!(!app.show_cargo_ng);
    // NG が無いうちは「表示済み」にしない。あとで NG が出たら開けるようにするため。
    assert!(!app.cargo_ng_auto_shown);
}

#[test]
fn auto_open_cargo_ng_once_yields_to_an_open_overlay() {
    let mut app = app_with(vec![ng_repo("ng-repo")]);
    app.show_help = true;
    assert!(!app.auto_open_cargo_ng_once());
    assert!(!app.show_cargo_ng);
}

#[test]
fn jump_to_selected_cargo_ng_moves_the_cursor_and_closes() {
    let mut app = app_with(vec![ok_repo("ok-repo"), ng_repo("ng-repo")]);
    app.open_cargo_ng_from_repos();
    assert_eq!(app.cargo_ng_items.len(), 1);

    app.jump_to_selected_cargo_ng();
    assert!(!app.show_cargo_ng);
    assert_eq!(
        app.selected_repo().map(|repo| repo.name.as_str()),
        Some("ng-repo")
    );
}

#[test]
fn cargo_ng_move_stays_in_range() {
    let mut app = app_with(vec![ng_repo("a"), ng_repo("b")]);
    app.open_cargo_ng_from_repos();
    app.cargo_ng_move_down(10);
    assert_eq!(app.cargo_ng_selected, 1);
    app.cargo_ng_move_up(10);
    assert_eq!(app.cargo_ng_selected, 0);
}
