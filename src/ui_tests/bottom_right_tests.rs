use super::*;

fn make_app_for_bottom_right() -> App {
    App::new(make_config())
}

#[test]
fn bottom_right_box_flags_staging_only() {
    let mut app = make_app_for_bottom_right();
    let mut repo = make_repo("staging-only");
    repo.local_status = LocalStatus::Staging;
    app.repos = vec![repo];
    let (show_staging, show_cargo_hash) = bottom_right_box_flags(&app, 0);
    assert!(show_staging);
    assert!(show_cargo_hash);
}

#[test]
fn bottom_right_box_flags_modified_only() {
    let mut app = make_app_for_bottom_right();
    let mut repo = make_repo("modified-only");
    repo.local_status = LocalStatus::Modified;
    repo.staging_files = vec![" M file.txt".to_string()];
    app.repos = vec![repo];
    let (show_staging, show_cargo_hash) = bottom_right_box_flags(&app, 0);
    assert!(show_staging);
    assert!(show_cargo_hash);
}

#[test]
fn bottom_right_box_flags_conflict_only() {
    let mut app = make_app_for_bottom_right();
    let mut repo = make_repo("conflict-only");
    repo.local_status = LocalStatus::Conflict;
    repo.staging_files = vec!["UU file.txt".to_string()];
    app.repos = vec![repo];
    let (show_staging, show_cargo_hash) = bottom_right_box_flags(&app, 0);
    assert!(show_staging);
    assert!(show_cargo_hash);
}

#[test]
fn bottom_right_box_flags_cargo_old_only() {
    let mut app = make_app_for_bottom_right();
    let mut repo = make_repo("cargo-old-only");
    repo.cargo_install = Some(true);
    app.repos = vec![repo];
    let (show_staging, show_cargo_hash) = bottom_right_box_flags(&app, 0);
    assert!(!show_staging);
    assert!(show_cargo_hash);
}

#[test]
fn bottom_right_box_flags_staging_and_cargo_old() {
    let mut app = make_app_for_bottom_right();
    let mut repo = make_repo("both");
    repo.local_status = LocalStatus::Staging;
    repo.cargo_install = Some(false);
    app.repos = vec![repo];
    let (show_staging, show_cargo_hash) = bottom_right_box_flags(&app, 0);
    assert!(show_staging);
    assert!(show_cargo_hash);
}

#[test]
fn bottom_right_stack_offsets_empty() {
    let offsets = bottom_right_stack_offsets(&[]);
    assert!(offsets.is_empty());
}

#[test]
fn bottom_right_stack_offsets_two_boxes() {
    let offsets = bottom_right_stack_offsets(&[5, 3]);
    assert_eq!(offsets, vec![0, 5]);
}

#[test]
fn bottom_right_stack_offsets_three_boxes() {
    let offsets = bottom_right_stack_offsets(&[5, 3, 2]);
    assert_eq!(offsets, vec![0, 5, 8]);
}

#[test]
fn bottom_right_boxes_order_staging_only() {
    let boxes = bottom_right_boxes(true, false);
    assert_eq!(boxes, vec![BottomRightBox::LocalChanges]);
}

#[test]
fn bottom_right_boxes_order_cargo_old_only() {
    let boxes = bottom_right_boxes(false, true);
    assert_eq!(boxes, vec![BottomRightBox::CargoHash]);
}

#[test]
fn bottom_right_boxes_order_both() {
    let boxes = bottom_right_boxes(true, true);
    assert_eq!(
        boxes,
        vec![BottomRightBox::CargoHash, BottomRightBox::LocalChanges]
    );
}

#[test]
fn draw_ui_shows_cargo_hash_box_with_local_remote_installed_order() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.repos[0].cargo_install = Some(true);
    app.repos[0].cargo_checked_at = "localhash123".to_string();
    app.repos[0].cargo_remote_hash = "remotehash456".to_string();
    app.repos[0].cargo_installed_hash = "installed789".to_string();

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let area = terminal.backend().buffer().area;
    let mut rendered = Vec::new();
    for y in 0..area.height {
        let mut line = String::new();
        for x in 0..area.width {
            line.push_str(terminal.backend().buffer()[(x, y)].symbol());
        }
        rendered.push(line);
    }
    let rendered = rendered.join("\n");
    let local_idx = rendered.find("localhash123").unwrap();
    let remote_idx = rendered.find("remotehash456").unwrap();
    let installed_idx = rendered.find("installed789").unwrap();

    assert!(rendered.contains("cgo: commit hash"));
    assert!(local_idx < remote_idx);
    assert!(remote_idx < installed_idx);
}
