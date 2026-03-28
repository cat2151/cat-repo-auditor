use super::*;

// ── local_check_cell ──────────────────────────────────────────────────────────

#[test]
fn local_check_cell_none_with_local_shows_question_mark() {
    let (s, c) = local_check_cell(false, None, MK_YELLOW);
    assert_eq!(s, "?");
    assert_eq!(c, MK_ORANGE);
}

#[test]
fn local_check_cell_none_no_git_shows_gray_dash() {
    let (s, c) = local_check_cell(true, None, MK_YELLOW);
    assert_eq!(s, "-");
    assert_eq!(c, MK_COMMENT);
}

#[test]
fn local_check_cell_some_true_shows_checkmark_with_true_col() {
    let (s, c) = local_check_cell(false, Some(true), MK_YELLOW);
    assert_eq!(s, "✔");
    assert_eq!(c, MK_YELLOW);
}

#[test]
fn local_check_cell_some_true_no_git_still_shows_checkmark() {
    // A previously cached true value should still show even if no local git
    let (s, c) = local_check_cell(true, Some(true), MK_PURPLE);
    assert_eq!(s, "✔");
    assert_eq!(c, MK_PURPLE);
}

#[test]
fn local_check_cell_some_false_shows_cross_gray() {
    let (s, c) = local_check_cell(false, Some(false), MK_YELLOW);
    assert_eq!(s, "✘");
    assert_eq!(c, MK_COMMENT);
}

#[test]
fn local_check_cell_some_false_no_git_shows_cross_gray() {
    let (s, c) = local_check_cell(true, Some(false), MK_YELLOW);
    assert_eq!(s, "✘");
    assert_eq!(c, MK_COMMENT);
}

#[test]
fn window_color_keeps_color_when_window_is_focused() {
    assert_eq!(window_color(true, MK_RED), MK_RED);
}

#[test]
fn window_color_converts_rgb_to_dim_grayscale_when_window_is_unfocused() {
    assert_eq!(
        window_color(false, MK_RED),
        ratatui::style::Color::Rgb(65, 65, 65)
    );
    assert_eq!(
        window_color(false, MK_BG_SEL),
        ratatui::style::Color::Rgb(42, 42, 42)
    );
}

#[test]
fn draw_ui_dims_active_border_when_window_is_unfocused() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(false);

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let cell = &terminal.backend().buffer()[(0, 1)];
    assert_eq!(cell.symbol(), "┌");
    assert_eq!(cell.fg, window_color(false, MK_COMMENT));
}

#[test]
fn draw_ui_keeps_active_border_color_when_window_is_focused() {
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    let cell = &terminal.backend().buffer()[(0, 1)];
    assert_eq!(cell.symbol(), "┌");
    assert_eq!(cell.fg, MK_CYAN);
}

#[test]
fn draw_ui_refreshes_log_lines_from_file_when_log_panel_is_visible() {
    let tmp = TempDirGuard::new("ui_log_refresh");
    let log_path = tmp.path().join("log.txt");
    std::fs::write(&log_path, "disk line 1\ndisk line 2\n").unwrap();

    let mut app = make_test_app_with_focus(true);
    app.show_log = true;
    app.log_lines = vec![String::from("stale line")];
    refresh_visible_log_panel(&mut app, &log_path);

    assert_eq!(app.log_lines, vec!["disk line 1", "disk line 2"]);
}

#[test]
fn refresh_visible_log_panel_does_not_reload_when_log_panel_is_hidden() {
    let tmp = TempDirGuard::new("ui_log_hidden");
    let log_path = tmp.path().join("log.txt");
    std::fs::write(&log_path, "disk line 1\ndisk line 2\n").unwrap();

    let mut app = make_test_app_with_focus(true);
    app.show_log = false;
    app.log_lines = vec![String::from("stale line")];
    refresh_visible_log_panel(&mut app, &log_path);

    assert_eq!(app.log_lines, vec!["stale line"]);
    assert!(app.log_last_modified.is_none());
}

#[test]
fn refresh_visible_log_panel_caps_reloaded_log_history() {
    let tmp = TempDirGuard::new("ui_log_refresh_cap");
    let log_path = tmp.path().join("log.txt");
    let content = (0..2_100)
        .map(|i| format!("line{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&log_path, format!("{content}\n")).unwrap();

    let mut app = make_test_app_with_focus(true);
    app.show_log = true;
    refresh_visible_log_panel(&mut app, &log_path);

    assert_eq!(app.log_lines.len(), 2_000);
    assert_eq!(app.log_lines.first().unwrap(), "line100");
    assert_eq!(app.log_lines.last().unwrap(), "line2099");
}

// ── background task spinner ───────────────────────────────────────────────────

#[test]
fn build_tasks_display_empty_when_no_tasks() {
    let s = build_tasks_display(&[], 0);
    assert_eq!(s, "");
}

#[test]
fn build_tasks_display_includes_spinner_and_progress() {
    let tasks = vec![("gh↓", 2, 0), ("scan", 3, 76)];
    let s = build_tasks_display(&tasks, 0);
    assert_eq!(s, "  ⠋ gh↓2  scan3/76");
}

#[test]
fn build_tasks_display_spinner_changes_by_250ms() {
    let tasks = vec![("scan", 1, 2)];
    let a = build_tasks_display(&tasks, 0);
    let b = build_tasks_display(&tasks, SPINNER_FRAME_MS);
    assert_ne!(a, b);
}

#[test]
fn build_tasks_display_spinner_cycles_through_requested_frames() {
    let tasks = vec![("scan", 1, 2)];
    for (idx, expected) in SPINNER_FRAMES.iter().enumerate() {
        let s = build_tasks_display(&tasks, (idx as u64) * SPINNER_FRAME_MS);
        assert!(s.starts_with(&format!("  {} ", expected)));
    }
}

#[test]
fn build_tasks_display_spinner_wraps_after_full_cycle() {
    let tasks = vec![("scan", 1, 2)];
    let a = build_tasks_display(&tasks, 0);
    let b = build_tasks_display(&tasks, (SPINNER_FRAMES.len() as u64) * SPINNER_FRAME_MS);
    assert_eq!(a, b);
}

#[test]
fn draw_ui_shows_workflow_repo_exist_overlay() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.open_workflow_repo_exist(vec![
        crate::github_local::WorkflowRepoExistCheck {
            workflow_file: String::from("call-a.yml"),
            installed_repos: vec![String::from("repo-a")],
            missing_repos: vec![String::from("repo-b"), String::from("repo-c")],
        },
        crate::github_local::WorkflowRepoExistCheck {
            workflow_file: String::from("call-b.yml"),
            installed_repos: vec![],
            missing_repos: vec![String::from("repo-a")],
        },
    ]);

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

    assert!(rendered.contains("workflow repo exist check"));
    assert!(rendered.contains("call-a.yml"));
    assert!(rendered.contains("repo-a"));
    assert!(rendered.contains("repo-b"));
    assert!(rendered.contains("repo-c"));
}

#[test]
fn draw_ui_shows_empty_workflow_repo_exist_overlay_message() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.open_workflow_repo_exist(vec![]);

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

    assert!(rendered.contains("workflow repo exist check"));
    assert!(rendered.contains("call* workflow"));
    assert!(rendered.contains("(none)"));
}
