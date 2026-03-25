use super::*;

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
    let sep_count = rows
        .iter()
        .filter(|r| matches!(r, RepoRow::Separator(_)))
        .count();
    let repo_count = rows
        .iter()
        .filter(|r| matches!(r, RepoRow::Repo(_)))
        .count();
    assert_eq!(
        sep_count, 0,
        "no separator expected when all repos are in group 0"
    );
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
    let sep_count = rows
        .iter()
        .filter(|r| matches!(r, RepoRow::Separator(_)))
        .count();
    assert_eq!(
        sep_count, 1,
        "expected one separator between open-PR and no-PR groups"
    );
    // separator label should contain "no open PRs"
    let has_label = rows.iter().any(|r| {
        if let RepoRow::Separator(label) = r {
            label.contains("no open PRs")
        } else {
            false
        }
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
    let sep_count = rows
        .iter()
        .filter(|r| matches!(r, RepoRow::Separator(_)))
        .count();
    // private group (3) gets a separator
    assert!(
        sep_count >= 1,
        "expected at least one separator for private group"
    );
}

#[test]
fn build_rows_not_found_repos_get_separator() {
    let mut not_found = make_repo("missing");
    not_found.local_status = LocalStatus::NotFound;
    let mut found = make_repo("present");
    found.open_issues = 1;

    let repos = vec![found, not_found];
    let rows = build_rows(&repos);
    let sep_count = rows
        .iter()
        .filter(|r| matches!(r, RepoRow::Separator(_)))
        .count();
    assert!(sep_count >= 1, "expected separator for NotFound group");
}

#[test]
fn build_rows_preserves_repo_indices() {
    let repos = vec![make_repo("a"), make_repo("b"), make_repo("c")];
    let rows = build_rows(&repos);
    let indices: Vec<usize> = rows
        .iter()
        .filter_map(|r| {
            if let RepoRow::Repo(i) = r {
                Some(*i)
            } else {
                None
            }
        })
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
    repo.issues = vec![
        make_issue(1, "bug", "owner/a"),
        make_issue(2, "feat", "owner/a"),
    ];
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
    assert!(
        !items[0].is_child,
        "should be standalone since issue 99 is not open"
    );
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
