use super::*;
use crate::{app::App, config::Config};
use crate::github::{IssueOrPr, LocalStatus, RepoInfo};
use ratatui::{backend::TestBackend, Terminal};
use std::sync::{Mutex, OnceLock};

// ── helpers ──────────────────────────────────────────────────────────────────

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

fn make_config() -> Config {
    Config {
        owner: "owner".to_string(),
        local_base_dir: ".".to_string(),
        app_run_dir: None,
        auto_pull: false,
    }
}

fn make_test_app_with_focus(window_focused: bool) -> App {
    let mut app = App::new(make_config());
    let mut repo = make_repo("focus-test");
    repo.open_prs = 1;
    repo.open_issues = 2;
    app.repos = vec![repo];
    app.window_focused = window_focused;
    app.rebuild_rows();
    app
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();
    let dir_name = format!("{prefix}_{pid}_{nanos}");
    let dir = std::env::temp_dir().join(dir_name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct TempDirGuard {
    path: std::path::PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        Self {
            path: unique_temp_dir(prefix),
        }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).unwrap();
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct ScopedEnvVar {
    key: &'static str,
    value: Option<std::ffi::OsString>,
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: &std::path::Path) -> Self {
        let old = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, value: old }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        if let Some(value) = &self.value {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

// ── build_rows ────────────────────────────────────────────────────────────────

#[test]
fn build_rows_single_group_no_separator() {
    // Repos with open_prs=1 land in group 0; when all repos are in the same
    // group (group 0) build_rows must not insert any separator.
    let mut a = make_repo("a");
    a.open_prs = 1;
    let mut b = make_repo("b");
    b.open_prs = 1;
    let repos = vec![a, b];
    let rows = build_rows(&repos);
    let sep_count = rows.iter().filter(|r| matches!(r, RepoRow::Separator(_))).count();
    let repo_count = rows.iter().filter(|r| matches!(r, RepoRow::Repo(_))).count();
    assert_eq!(sep_count, 0, "no separator expected when all repos are in group 0");
    assert_eq!(repo_count, 2);
}

#[test]
fn build_rows_no_open_prs_repos_get_separator() {
    // Repos with open_issues>0 but open_prs=0 land in group 1 ("no open PRs")
    // and get a separator.
    let mut with_pr = make_repo("has-pr");
    with_pr.open_prs = 1;
    let mut no_pr = make_repo("no-pr");
    no_pr.open_issues = 1;

    let repos = vec![with_pr, no_pr];
    let rows = build_rows(&repos);
    let sep_count = rows.iter().filter(|r| matches!(r, RepoRow::Separator(_))).count();
    assert_eq!(sep_count, 1, "expected one separator between open-PR and no-PR groups");
    // separator label should contain "no open PRs"
    let has_label = rows.iter().any(|r| {
        if let RepoRow::Separator(label) = r { label.contains("no open PRs") } else { false }
    });
    assert!(has_label, "separator label should mention 'no open PRs'");
}

#[test]
fn build_rows_private_repos_get_separator() {
    let mut private_repo = make_repo("private");
    private_repo.is_private = true;
    let mut public_repo = make_repo("public");
    public_repo.open_issues = 1; // group 0

    let repos = vec![public_repo, private_repo];
    let rows = build_rows(&repos);
    let sep_count = rows.iter().filter(|r| matches!(r, RepoRow::Separator(_))).count();
    // private group (3) gets a separator
    assert!(sep_count >= 1, "expected at least one separator for private group");
}

#[test]
fn build_rows_not_found_repos_get_separator() {
    let mut not_found = make_repo("missing");
    not_found.local_status = LocalStatus::NotFound;
    let mut found = make_repo("present");
    found.open_issues = 1;

    let repos = vec![found, not_found];
    let rows = build_rows(&repos);
    let sep_count = rows.iter().filter(|r| matches!(r, RepoRow::Separator(_))).count();
    assert!(sep_count >= 1, "expected separator for NotFound group");
}

#[test]
fn build_rows_preserves_repo_indices() {
    let repos = vec![make_repo("a"), make_repo("b"), make_repo("c")];
    let rows = build_rows(&repos);
    let indices: Vec<usize> = rows.iter()
        .filter_map(|r| if let RepoRow::Repo(i) = r { Some(*i) } else { None })
        .collect();
    assert_eq!(indices.len(), 3);
    // indices must be valid indices into repos
    for i in &indices {
        assert!(*i < repos.len());
    }
}

// ── build_detail_items ────────────────────────────────────────────────────────

#[test]
fn build_detail_items_issue_only() {
    let mut repo = make_repo("a");
    repo.issues = vec![make_issue(1, "bug", "owner/a"), make_issue(2, "feat", "owner/a")];
    let items = build_detail_items(&repo);
    assert_eq!(items.len(), 2);
    assert!(!items[0].is_pr);
    assert!(!items[1].is_pr);
}

#[test]
fn build_detail_items_standalone_pr() {
    let mut repo = make_repo("a");
    repo.prs = vec![make_pr(10, "pr", "owner/a", None)];
    let items = build_detail_items(&repo);
    assert_eq!(items.len(), 1);
    assert!(items[0].is_pr);
    assert!(!items[0].is_child);
}

#[test]
fn build_detail_items_pr_linked_to_issue_appears_as_child() {
    let mut repo = make_repo("a");
    repo.issues = vec![make_issue(1, "bug", "owner/a")];
    repo.prs = vec![make_pr(2, "fix bug", "owner/a", Some(1))];
    let items = build_detail_items(&repo);
    // issue first, then its PR child
    assert_eq!(items.len(), 2);
    assert!(!items[0].is_pr);
    assert_eq!(items[0].number, 1);
    assert!(items[1].is_pr);
    assert!(items[1].is_child);
    assert_eq!(items[1].number, 2);
}

#[test]
fn build_detail_items_pr_closes_nonexistent_issue_is_standalone() {
    let mut repo = make_repo("a");
    // PR closes issue 99, but issue 99 is not in repo.issues
    repo.prs = vec![make_pr(5, "stale fix", "owner/a", Some(99))];
    let items = build_detail_items(&repo);
    assert_eq!(items.len(), 1);
    assert!(items[0].is_pr);
    assert!(!items[0].is_child, "should be standalone since issue 99 is not open");
}

#[test]
fn build_detail_items_multiple_prs_for_one_issue() {
    let mut repo = make_repo("a");
    repo.issues = vec![make_issue(1, "big bug", "owner/a")];
    repo.prs = vec![
        make_pr(3, "fix attempt 1", "owner/a", Some(1)),
        make_pr(2, "fix attempt 2", "owner/a", Some(1)),
    ];
    let items = build_detail_items(&repo);
    // issue + 2 child PRs
    assert_eq!(items.len(), 3);
    assert!(!items[0].is_pr);
    // child PRs are sorted by number
    assert!(items[1].is_child);
    assert_eq!(items[1].number, 2);
    assert!(items[2].is_child);
    assert_eq!(items[2].number, 3);
}

#[test]
fn build_detail_items_empty_repo() {
    let repo = make_repo("empty");
    let items = build_detail_items(&repo);
    assert!(items.is_empty());
}

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
    assert_eq!(window_color(false, MK_RED), ratatui::style::Color::Rgb(65, 65, 65));
    assert_eq!(window_color(false, MK_BG_SEL), ratatui::style::Color::Rgb(42, 42, 42));
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
    let _lock_guard = env_lock().lock().unwrap();
    let tmp = TempDirGuard::new("ui_log_refresh");
    let _xdg = ScopedEnvVar::set("XDG_CONFIG_HOME", tmp.path());

    let log_path = Config::log_path();
    std::fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    std::fs::write(&log_path, "disk line 1\ndisk line 2\n").unwrap();

    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = make_test_app_with_focus(true);
    app.show_log = true;
    app.log_lines = vec![String::from("stale line")];

    terminal.draw(|f| draw_ui(f, &mut app)).unwrap();

    assert_eq!(app.log_lines, vec!["disk line 1", "disk line 2"]);
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
fn bottom_right_box_flags_staging_only() {
    let mut app = crate::app::App::new(crate::config::Config {
        owner: "owner".to_string(),
        local_base_dir: ".".to_string(),
        app_run_dir: None,
        auto_pull: false,
    });
    let mut repo = make_repo("staging-only");
    repo.local_status = LocalStatus::Staging;
    app.repos = vec![repo];
    let (show_staging, show_cargo_old) = bottom_right_box_flags(&app, 0);
    assert!(show_staging);
    assert!(!show_cargo_old);
}

#[test]
fn bottom_right_box_flags_modified_only() {
    let mut app = crate::app::App::new(crate::config::Config {
        owner: "owner".to_string(),
        local_base_dir: ".".to_string(),
        app_run_dir: None,
        auto_pull: false,
    });
    let mut repo = make_repo("modified-only");
    repo.local_status = LocalStatus::Modified;
    repo.staging_files = vec![" M file.txt".to_string()];
    app.repos = vec![repo];
    let (show_staging, show_cargo_old) = bottom_right_box_flags(&app, 0);
    assert!(show_staging);
    assert!(!show_cargo_old);
}

#[test]
fn bottom_right_box_flags_conflict_only() {
    let mut app = crate::app::App::new(crate::config::Config {
        owner: "owner".to_string(),
        local_base_dir: ".".to_string(),
        app_run_dir: None,
        auto_pull: false,
    });
    let mut repo = make_repo("conflict-only");
    repo.local_status = LocalStatus::Conflict;
    repo.staging_files = vec!["UU file.txt".to_string()];
    app.repos = vec![repo];
    let (show_staging, show_cargo_old) = bottom_right_box_flags(&app, 0);
    assert!(show_staging);
    assert!(!show_cargo_old);
}

#[test]
fn bottom_right_box_flags_cargo_old_only() {
    let mut app = crate::app::App::new(crate::config::Config {
        owner: "owner".to_string(),
        local_base_dir: ".".to_string(),
        app_run_dir: None,
        auto_pull: false,
    });
    let mut repo = make_repo("cargo-old-only");
    repo.cargo_install = Some(false);
    app.repos = vec![repo];
    let (show_staging, show_cargo_old) = bottom_right_box_flags(&app, 0);
    assert!(!show_staging);
    assert!(show_cargo_old);
}

#[test]
fn bottom_right_box_flags_staging_and_cargo_old() {
    let mut app = crate::app::App::new(crate::config::Config {
        owner: "owner".to_string(),
        local_base_dir: ".".to_string(),
        app_run_dir: None,
        auto_pull: false,
    });
    let mut repo = make_repo("both");
    repo.local_status = LocalStatus::Staging;
    repo.cargo_install = Some(false);
    app.repos = vec![repo];
    let (show_staging, show_cargo_old) = bottom_right_box_flags(&app, 0);
    assert!(show_staging);
    assert!(show_cargo_old);
}

#[test]
fn bottom_right_stack_offsets_empty() {
    let offsets = bottom_right_stack_offsets(&[]);
    assert!(offsets.is_empty());
}

#[test]
fn bottom_right_stack_offsets_two_boxes() {
    let offsets = bottom_right_stack_offsets(&[4, 3]);
    assert_eq!(offsets, vec![0, 4]);
}

#[test]
fn bottom_right_stack_offsets_three_boxes() {
    let offsets = bottom_right_stack_offsets(&[4, 3, 2]);
    assert_eq!(offsets, vec![0, 4, 7]);
}

#[test]
fn bottom_right_boxes_order_staging_only() {
    let boxes = bottom_right_boxes(true, false);
    assert_eq!(boxes, vec![BottomRightBox::LocalChanges]);
}

#[test]
fn bottom_right_boxes_order_cargo_old_only() {
    let boxes = bottom_right_boxes(false, true);
    assert_eq!(boxes, vec![BottomRightBox::CargoOld]);
}

#[test]
fn bottom_right_boxes_order_both() {
    let boxes = bottom_right_boxes(true, true);
    assert_eq!(boxes, vec![BottomRightBox::CargoOld, BottomRightBox::LocalChanges]);
}
