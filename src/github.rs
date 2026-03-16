use crate::{config::Config, history::History};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::process::Command;

// ──────────────────────────────────────────────
// Public types
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IssueOrPr {
    pub title: String,
    pub updated_at: String,
    pub updated_at_raw: String,
    pub number: u64,
    pub repo_full: String,
    pub is_pr: bool,
    pub closes_issue: Option<u64>,
}

impl IssueOrPr {
    pub fn url(&self) -> String {
        let kind = if self.is_pr { "pull" } else { "issues" };
        format!("https://github.com/{}/{}/{}", self.repo_full, kind, self.number)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub full_name: String,
    pub updated_at: String,
    pub updated_at_raw: String,
    pub open_issues: u64,
    pub open_prs: u64,
    pub is_private: bool,
    pub local_status: LocalStatus,
    pub has_local_git: bool,
    pub staging_files: Vec<String>,
    pub issues: Vec<IssueOrPr>,
    pub prs: Vec<IssueOrPr>,
    /// README.ja.md existence. None = unchecked.
    #[serde(default)]
    pub readme_ja: Option<bool>,
    /// updated_at_raw when readme_ja was last checked
    #[serde(default)]
    pub readme_ja_checked_at: String,

    /// README.ja.md contains a self-referencing badge/link ("README.ja.md" text in the file)
    #[serde(default)]
    pub readme_ja_badge: Option<bool>,
    /// local HEAD hash when readme_ja_badge was last checked
    #[serde(default)]
    pub readme_ja_badge_checked_at: String,

    #[serde(default)]
    pub pages: Option<bool>,
    #[serde(default)]
    pub pages_checked_at: String,

    #[serde(default)]
    pub deepwiki: Option<bool>,
    #[serde(default)]
    pub deepwiki_checked_at: String,

    /// None = repo not found in .crates2.json (not installed via cargo install --git)
    /// Some(true) = installed hash == local HEAD, Some(false) = stale
    #[serde(default)]
    pub cargo_install: Option<bool>,
    /// local git HEAD hash when cargo_install was last checked
    #[serde(default)]
    pub cargo_checked_at: String,

    /// All 3 required workflow yml files present in .github/workflows/
    #[serde(default)]
    pub wf_workflows: Option<bool>,
    /// local HEAD hash when wf_workflows was last checked
    #[serde(default)]
    pub wf_checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LocalStatus {
    Pullable,
    Clean,
    Staging,
    Other,
    NotFound,
    NoGit,
}

impl std::fmt::Display for LocalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocalStatus::Pullable => write!(f, "pullable"),
            LocalStatus::Clean    => write!(f, "clean"),
            LocalStatus::Staging  => write!(f, "staging"),
            LocalStatus::Other    => write!(f, "other"),
            LocalStatus::NotFound => write!(f, "-"),
            LocalStatus::NoGit    => write!(f, "no-git"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub remaining: u64,
    pub limit: u64,
    pub reset_at: String,
}

// ──────────────────────────────────────────────
// GraphQL response shapes
// ──────────────────────────────────────────────

#[derive(Deserialize)]
struct GqlResponse { data: GqlData }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlData {
    repository_owner: RepositoryOwner,
    rate_limit: GqlRateLimit,
}

#[derive(Deserialize)]
struct RepositoryOwner { repositories: RepositoryConnection }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryConnection {
    nodes: Vec<GqlRepo>,
    page_info: PageInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlRepo {
    name: String,
    name_with_owner: String,
    updated_at: String,
    is_fork: bool,
    is_archived: bool,
    is_private: bool,
    issues: IssueConnection,
    pull_requests: PrConnection,
}

#[derive(Deserialize)]
struct IssueConnection {
    #[serde(rename = "totalCount")] total_count: u64,
    nodes: Vec<GqlIssue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlIssue { title: String, number: u64, updated_at: String }

#[derive(Deserialize)]
struct PrConnection {
    #[serde(rename = "totalCount")] total_count: u64,
    nodes: Vec<GqlPr>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlPr {
    title: String,
    number: u64,
    updated_at: String,
    closing_issues_references: ClosingIssues,
}

#[derive(Deserialize, Default)]
struct ClosingIssues { nodes: Vec<ClosingIssueNode> }

#[derive(Deserialize)]
struct ClosingIssueNode { number: u64 }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlRateLimit { remaining: u64, limit: u64, reset_at: String }

// ──────────────────────────────────────────────
// Progress channel
// ──────────────────────────────────────────────

pub enum FetchProgress {
    Status(String),
    /// Structured progress for background task display: (tag, cur, total)
    /// tag examples: "gh↓", "scan", "pull", "chk"
    PhaseProgress { tag: &'static str, cur: usize, total: usize },
    /// Signal that a specific repo is currently being checked (for UI feedback)
    CheckingRepo(String),
    /// Incremental update per repo after phase-3 checks
    ExistenceUpdate {
        name:                   String,
        readme_ja:              Option<bool>,
        readme_ja_cat:          String,
        readme_ja_badge:        Option<bool>,
        readme_ja_badge_cat:    String,
        pages:                  Option<bool>,
        pages_cat:              String,
        deepwiki:               Option<bool>,
        deepwiki_cat:           String,
        cargo_install:          Option<bool>,
        cargo_cat:              String,
        wf_workflows:           Option<bool>,
        wf_cat:                 String,
    },
    Done(Result<(Vec<RepoInfo>, RateLimit)>),
}

pub fn fetch_repos_with_progress(
    config: Config,
    mut history: History,
    tx: std::sync::mpsc::Sender<FetchProgress>,
) {
    // Phase 1: fetch repo list
    let result = do_fetch(&config, &mut history, &tx);
    match result {
        Err(e) => { let _ = tx.send(FetchProgress::Done(Err(e))); }
        Ok((mut repos, rl)) => {
            // Phase 2: auto-pull pullable repos (only when config.auto_pull = true)
            let pullable: Vec<String> = if config.auto_pull {
                repos.iter()
                    .filter(|r| r.local_status == LocalStatus::Pullable)
                    .map(|r| r.name.clone())
                    .collect()
            } else { vec![] };
            if !pullable.is_empty() {
                let total = pullable.len();
                for (i, name) in pullable.iter().enumerate() {
                    let _ = tx.send(FetchProgress::PhaseProgress { tag: "pull", cur: i + 1, total });
                    let _ = git_pull(&config.local_base_dir, name);
                }
                let _ = tx.send(FetchProgress::Status(String::from("Refreshing after auto-pull…")));
                match do_fetch(&config, &mut history, &tx) {
                    Ok((r2, rl2)) => { repos = r2; let _ = tx.send(FetchProgress::Done(Ok((repos.clone(), rl2)))); }
                    Err(e)        => { let _ = tx.send(FetchProgress::Done(Err(e))); return; }
                }
            } else {
                let _ = tx.send(FetchProgress::Done(Ok((repos.clone(), rl))));
            }

            // Phase 3: per-field independent checked_at.
            // Each field is rechecked only when its own checked_at is stale.
            // cargo_checked_at stores the local HEAD hash → rechecks only on new commit.
            let owner = config.owner.clone();


            // Collect local HEAD hashes once (cheap, no network)
            let local_heads: std::collections::HashMap<String, String> = repos.iter()
                .filter(|r| r.has_local_git)
                .filter_map(|r| {
                    let path = format!("{}/{}", config.local_base_dir.trim_end_matches(|c| c == '/' || c == '\\'), r.name);
                    let out = std::process::Command::new("git")
                        .args(["-C", &path, "rev-parse", "HEAD"])
                        .output().ok()?;
                    if !out.status.success() { return None; }
                    Some((r.name.clone(), String::from_utf8_lossy(&out.stdout).trim().to_string()))
                })
                .collect();

            // Build per-repo check tasks: only repos that need at least one field updated
            let to_check: Vec<String> = repos.iter()
                .filter(|r| {
                    let cat = &r.updated_at_raw;
                    let local_head = local_heads.get(&r.name).map(|s| s.as_str()).unwrap_or("");
                    r.readme_ja_checked_at       != *cat
                    || r.readme_ja_badge_checked_at != local_head
                    || r.pages_checked_at            != *cat
                    || r.deepwiki_checked_at         != local_head
                    || r.cargo_checked_at            != local_head
                    || r.wf_checked_at               != local_head
                })
                .map(|r| r.name.clone())
                .collect();

            if to_check.is_empty() {
                return;
            }
            let total_check = to_check.len();

            for (i, name) in to_check.iter().enumerate() {
                let repo = repos.iter().find(|r| &r.name == name).unwrap();
                let cat = repo.updated_at_raw.clone();
                let local_head = local_heads.get(name).cloned().unwrap_or_default();

                let needs_readme       = repo.readme_ja_checked_at       != cat;
                let needs_ja_badge     = repo.readme_ja_badge_checked_at != local_head;
                let needs_pages        = repo.pages_checked_at            != cat;
                let needs_deepwiki     = repo.deepwiki_checked_at         != local_head;
                let needs_cargo        = repo.cargo_checked_at            != local_head;
                let needs_wf           = repo.wf_checked_at               != local_head;


                // Signal UI that this repo is being checked
                let _ = tx.send(FetchProgress::CheckingRepo(name.clone()));
                let _ = tx.send(FetchProgress::PhaseProgress { tag: "chk", cur: i + 1, total: total_check });

                let (readme_ja, readme_ja_cat) = if needs_readme {
                    let v = check_file_exists(&owner, name, "README.ja.md");
                    (Some(v), cat.clone())
                } else {
                    (repo.readme_ja, repo.readme_ja_checked_at.clone())
                };

                let (readme_ja_badge, readme_ja_badge_cat) = if needs_ja_badge {
                    let v = check_readme_ja_badge(&config.local_base_dir, name);
                    (Some(v), local_head.clone())
                } else {
                    (repo.readme_ja_badge, repo.readme_ja_badge_checked_at.clone())
                };

                let (pages, pages_cat) = if needs_pages {
                    let v = check_pages_exists(&owner, name);
                    (Some(v), cat.clone())
                } else {
                    (repo.pages, repo.pages_checked_at.clone())
                };

                let (deepwiki, deepwiki_cat) = if needs_deepwiki {
                    let v = check_deepwiki_exists(&config.local_base_dir, name);
                    (Some(v), local_head.clone())
                } else {
                    (repo.deepwiki, repo.deepwiki_checked_at.clone())
                };

                let (cargo_install, cargo_cat) = if needs_cargo {
                    let v = check_cargo_git_install(&owner, name, &config.local_base_dir);
                    (v, local_head.clone())
                } else {
                    (repo.cargo_install, repo.cargo_checked_at.clone())
                };

                let (wf_workflows, wf_cat) = if needs_wf {
                    let v = check_workflows(&config.local_base_dir, name);
                    (Some(v), local_head.clone())
                } else {
                    (repo.wf_workflows, repo.wf_checked_at.clone())
                };

                if let Some(r) = history.repos.iter_mut().find(|r| &r.name == name) {
                    r.readme_ja                    = readme_ja;
                    r.readme_ja_checked_at         = readme_ja_cat.clone();
                    r.readme_ja_badge              = readme_ja_badge;
                    r.readme_ja_badge_checked_at   = readme_ja_badge_cat.clone();
                    r.pages                        = pages;
                    r.pages_checked_at             = pages_cat.clone();
                    r.deepwiki                     = deepwiki;
                    r.deepwiki_checked_at          = deepwiki_cat.clone();
                    r.cargo_install                = cargo_install;
                    r.cargo_checked_at             = cargo_cat.clone();
                    r.wf_workflows                 = wf_workflows;
                    r.wf_checked_at                = wf_cat.clone();
                }

                let _ = tx.send(FetchProgress::ExistenceUpdate {
                    name:                name.clone(),
                    readme_ja,           readme_ja_cat,
                    readme_ja_badge,     readme_ja_badge_cat,
                    pages,               pages_cat,
                    deepwiki,            deepwiki_cat,
                    cargo_install,       cargo_cat,
                    wf_workflows,        wf_cat,
                });
            }
            // Clear progress indicators
            let _ = tx.send(FetchProgress::CheckingRepo(String::new()));
            let _ = tx.send(FetchProgress::PhaseProgress { tag: "chk", cur: 0, total: 0 });
            history.save(&crate::config::Config::history_path().to_string_lossy()).ok();
        }
    }
}

// ──────────────────────────────────────────────
// Fetch
// ──────────────────────────────────────────────

fn do_fetch(
    config: &Config,
    history: &mut History,
    tx: &std::sync::mpsc::Sender<FetchProgress>,
) -> Result<(Vec<RepoInfo>, RateLimit)> {
    let owner = &config.owner;
    let etag_key = format!("repos:{owner}");

    let mut all_repos: Vec<GqlRepo> = vec![];
    let mut cursor: Option<String> = None;
    #[allow(unused_assignments)]
    let mut rate_limit_info: Option<RateLimit> = None;
    let mut page_num = 0u32;

    loop {
        page_num += 1;
        let _ = tx.send(FetchProgress::PhaseProgress { tag: "gh↓", cur: page_num as usize, total: 0 });

        let after_clause = match &cursor {
            Some(c) => format!(r#", after: "{}""#, c),
            None => String::new(),
        };

        let query = format!(
            r#"query {{
  repositoryOwner(login: "{owner}") {{
    repositories(first: 100, orderBy: {{field: UPDATED_AT, direction: DESC}}{after_clause}) {{
      nodes {{
        name
        nameWithOwner
        updatedAt
        isFork
        isArchived
        isPrivate
        issues(states: OPEN, first: 20, orderBy: {{field: UPDATED_AT, direction: DESC}}) {{
          totalCount
          nodes {{ title number updatedAt }}
        }}
        pullRequests(states: OPEN, first: 20, orderBy: {{field: UPDATED_AT, direction: DESC}}) {{
          totalCount
          nodes {{ title number updatedAt closingIssuesReferences(first: 2) {{ nodes {{ number }} }} }}
        }}
      }}
      pageInfo {{ hasNextPage endCursor }}
    }}
  }}
  rateLimit {{ remaining limit resetAt }}
}}"#
        );

        let output = run_gh_graphql(&query)?;
        let response_hash = format!("{:x}", fnv1a(&output));

        let gql: GqlResponse =
            serde_json::from_str(&output).context("Failed to parse GraphQL response")?;

        let rl = &gql.data.rate_limit;
        rate_limit_info = Some(RateLimit {
            remaining: rl.remaining,
            limit: rl.limit,
            reset_at: rl.reset_at.clone(),
        });

        if cursor.is_none() {
            if let Some(cached) = history.etags.get(&etag_key) {
                if *cached == response_hash && !history.repos.is_empty() {
                    let rl_out = rate_limit_info.unwrap();
                    history.rate_limit = Some(rl_out.clone());
                    history.save(&crate::config::Config::history_path().to_string_lossy()).ok();
                    return Ok((history.repos.clone(), rl_out));
                }
            }
            history.etags.insert(etag_key.clone(), response_hash);
        }

        let page_info = &gql.data.repository_owner.repositories.page_info;
        let has_next  = page_info.has_next_page;
        let end_cursor = page_info.end_cursor.clone();
        all_repos.extend(gql.data.repository_owner.repositories.nodes);
        if has_next { cursor = end_cursor; } else { break; }
    }

    let filtered: Vec<GqlRepo> = all_repos.into_iter()
        .filter(|r| !r.is_fork && !r.is_archived)
        .collect();

    let total = filtered.len();
    let filtered_vec: Vec<GqlRepo> = filtered.into_iter().collect();
    let mut repo_infos: Vec<RepoInfo> = Vec::with_capacity(total);
    for (scan_i, r) in filtered_vec.into_iter().enumerate() {
        let _ = tx.send(FetchProgress::PhaseProgress {
            tag: "scan", cur: scan_i + 1, total,
        });
        let full_name = r.name_with_owner.clone();
        let (local_status, has_local_git, staging_files) =
            check_local_status_no_fetch(&config.local_base_dir, &r.name);
        let raw = r.updated_at.clone();
        let updated_at_raw = format_date_iso(&raw);

        // Carry over cached existence fields from history (each has its own checked_at)
        let (readme_ja, readme_ja_checked_at,
             readme_ja_badge, readme_ja_badge_checked_at,
             pages, pages_checked_at,
             deepwiki, deepwiki_checked_at,
             cargo_install, cargo_checked_at,
             wf_workflows, wf_checked_at) = history.repos.iter()
            .find(|h| h.name == r.name)
            .map(|h| (
                h.readme_ja,          h.readme_ja_checked_at.clone(),
                h.readme_ja_badge,    h.readme_ja_badge_checked_at.clone(),
                h.pages,              h.pages_checked_at.clone(),
                h.deepwiki,           h.deepwiki_checked_at.clone(),
                h.cargo_install,      h.cargo_checked_at.clone(),
                h.wf_workflows,       h.wf_checked_at.clone(),
            ))
            .unwrap_or((None, String::new(), None, String::new(),
                        None, String::new(), None, String::new(),
                        None, String::new(), None, String::new()));

        repo_infos.push(RepoInfo {
            name: r.name.clone(),
            full_name: full_name.clone(),
            updated_at: relative_date(&raw),
            updated_at_raw,
            open_issues: r.issues.total_count,
            open_prs: r.pull_requests.total_count,
            is_private: r.is_private,
            local_status,
            has_local_git,
            staging_files,
            readme_ja,            readme_ja_checked_at,
            readme_ja_badge,      readme_ja_badge_checked_at,
            pages,                pages_checked_at,
            deepwiki,             deepwiki_checked_at,
            cargo_install,        cargo_checked_at,
            wf_workflows,         wf_checked_at,
            issues: r.issues.nodes.into_iter().map(|i| {
                let raw_i = i.updated_at.clone();
                IssueOrPr {
                    title: i.title, number: i.number,
                    updated_at: relative_date(&raw_i),
                    updated_at_raw: format_date_iso(&raw_i),
                    repo_full: full_name.clone(),
                    is_pr: false, closes_issue: None,
                }
            }).collect(),
            prs: r.pull_requests.nodes.into_iter().map(|p| {
                let closes_issue = if p.closing_issues_references.nodes.len() == 1 {
                    Some(p.closing_issues_references.nodes[0].number)
                } else { None };
                let raw_p = p.updated_at.clone();
                IssueOrPr {
                    title: p.title, number: p.number,
                    updated_at: relative_date(&raw_p),
                    updated_at_raw: format_date_iso(&raw_p),
                    repo_full: full_name.clone(),
                    is_pr: true, closes_issue,
                }
            }).collect(),
        });
    } // end for scan loop

    fn group_key(r: &RepoInfo) -> u8 {
        if r.is_private                            { return 3; }
        if r.local_status == LocalStatus::NotFound { return 2; }
        if r.open_issues == 0 && r.open_prs == 0  { return 1; }
        0
    }

    repo_infos.sort_by(|a, b| {
        group_key(a).cmp(&group_key(b))
            .then(b.updated_at_raw.cmp(&a.updated_at_raw))
    });

    let rl_out = rate_limit_info.unwrap_or(RateLimit {
        remaining: 0, limit: 5000, reset_at: String::new(),
    });

    history.repos = repo_infos.clone();
    history.rate_limit = Some(rl_out.clone());
    history.save(&crate::config::Config::history_path().to_string_lossy()).ok();

    Ok((repo_infos, rl_out))
}

// ──────────────────────────────────────────────
// Existence checks via gh REST API
// ──────────────────────────────────────────────

/// Check if README.ja.md exists in the default branch root
fn check_file_exists(owner: &str, repo: &str, path: &str) -> bool {
    let endpoint = format!("/repos/{owner}/{repo}/contents/{path}");
    let out = Command::new("gh")
        .args(["api", &endpoint, "--silent"])
        .output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Check if GitHub Pages is enabled for the repo
fn check_pages_exists(owner: &str, repo: &str) -> bool {
    let endpoint = format!("/repos/{owner}/{repo}/pages");
    let out = Command::new("gh")
        .args(["api", &endpoint, "--silent"])
        .output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Check if DeepWiki page exists (HTTP GET, 404 = false)
/// Scan local README.ja.md for a deepwiki.com link.
/// Returns true if "deepwiki.com" appears anywhere in the file.
fn check_deepwiki_exists(base_dir: &str, repo_name: &str) -> bool {
    // Try README.ja.md first, then README.md as fallback
    for filename in &["README.ja.md", "README.md"] {
        let path = format!("{}/{}/{}",
            base_dir.trim_end_matches(|c| c == '/' || c == '\\'), repo_name, filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("deepwiki.com") {
                return true;
            }
        }
    }
    false
}

/// Check if all 3 required workflow yml files are present in .github/workflows/
fn check_workflows(base_dir: &str, repo_name: &str) -> bool {
    let base = base_dir.trim_end_matches(|c| c == '/' || c == '\\');
    let wf_dir = format!("{}/{}/.github/workflows", base, repo_name);
    let required = [
        "call-translate-readme.yml",
        "call-issue-note.yml",
        "call-check-large-files.yml",
    ];
    required.iter().all(|f| {
        std::path::Path::new(&format!("{}/{}", wf_dir, f)).exists()
    })
}

/// Launch an application with LeaveAlternateScreen/EnterAlternateScreen
/// to avoid terminal corruption (same pattern as lazygit).
pub fn launch_app(bin: &str, run_dir: &str) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    let status = Command::new(bin).current_dir(run_dir).status();
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    );
    match status {
        Ok(_) => Ok(()),
        Err(e) => bail!("launch failed: {e}"),
    }
}

/// Scan local README.ja.md for a self-referencing badge/link ("README.ja.md" text).
fn check_readme_ja_badge(base_dir: &str, repo_name: &str) -> bool {
    for filename in &["README.ja.md", "README.md"] {
        let path = format!("{}/{}/{}",
            base_dir.trim_end_matches(|c| c == '/' || c == '\\'), repo_name, filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("README.ja.md") {
                return true;
            }
        }
    }
    false
}

/// Get the installed binary names for a git-installed crate from .crates2.json.
/// Returns None if not found.
pub fn get_cargo_bins(owner: &str, repo_name: &str) -> Option<Vec<String>> {
    let crates2_path = std::env::var("CARGO_HOME")
        .map(|h| format!("{h}/.crates2.json"))
        .unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            format!("{home}/.cargo/.crates2.json")
        });

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    let needle = format!("git+https://github.com/{owner}/{repo_name}#");

    for (key, val) in installs {
        let src = key.trim_end_matches(')');
        if src.contains(needle.as_str()) {
            let bins = val.get("bins")?.as_array()?;
            return Some(
                bins.iter()
                    .filter_map(|b| b.as_str().map(|s| s.to_string()))
                    .collect()
            );
        }
    }
    None
}

/// Compare commit hash of `cargo install --git` entry against local HEAD.
/// Key format: "crate_name version (git+https://github.com/owner/repo#COMMIT_HASH)"
/// Returns:
///   None          – no git+…#hash entry for this repo found in .crates2.json
///   Some(true)    – installed hash == local HEAD
///   Some(false)   – installed hash != local HEAD (old)
pub fn check_cargo_git_install(owner: &str, repo_name: &str, base_dir: &str) -> Option<bool> {
    let crates2_path = std::env::var("CARGO_HOME")
        .map(|h| format!("{h}/.crates2.json"))
        .unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            format!("{home}/.cargo/.crates2.json")
        });

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    // Build the substring we look for in the key
    // e.g. "git+https://github.com/cat2151/clap-mml-render-tui#"
    let needle = format!("git+https://github.com/{owner}/{repo_name}#");

    let installed_hash = installs.keys()
        .find_map(|key| {
            // key: "name ver (git+https://...#HASH)"
            let src = key.trim_end_matches(')');
            let idx = src.find(needle.as_str())?;
            let hash_part = &src[idx + needle.len()..];
            if hash_part.is_empty() { return None; }
            Some(hash_part.to_string())
        })?;

    // Get local HEAD hash
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(|c| c == '/' || c == '\\'), repo_name);
    let out = Command::new("git")
        .args(["-C", &repo_path, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let local_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

    // Compare: installed_hash may be full (40 chars) – exact match
    Some(installed_hash == local_hash)
}

// ──────────────────────────────────────────────
// Local git status (no network)
// ──────────────────────────────────────────────

fn check_local_status_no_fetch(
    base_dir: &str,
    repo_name: &str,
) -> (LocalStatus, bool, Vec<String>) {
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(['/', '\\']), repo_name);

    if !std::path::Path::new(&repo_path).exists() {
        return (LocalStatus::NotFound, false, vec![]);
    }
    let git_dir = format!("{}/.git", repo_path);
    if !std::path::Path::new(&git_dir).exists() {
        return (LocalStatus::NoGit, false, vec![]);
    }

    let local  = Command::new("git").args(["-C", &repo_path, "rev-parse", "HEAD"]).output();
    let remote = Command::new("git").args(["-C", &repo_path, "rev-parse", "@{u}"]).output();
    let remote_ok = remote.as_ref().map(|r| r.status.success()).unwrap_or(false);
    let staging_files = get_staging_files(&repo_path);

    match (local, remote) {
        (Ok(l), Ok(r)) if l.status.success() && remote_ok => {
            let local_sha  = String::from_utf8_lossy(&l.stdout).trim().to_string();
            let remote_sha = String::from_utf8_lossy(&r.stdout).trim().to_string();

            if local_sha == remote_sha {
                if !staging_files.is_empty() {
                    return (LocalStatus::Staging, true, staging_files);
                }
                return (LocalStatus::Clean, true, vec![]);
            }

            let merge_base = Command::new("git")
                .args(["-C", &repo_path, "merge-base", "HEAD", "@{u}"])
                .output();

            if let Ok(mb) = merge_base {
                if mb.status.success() {
                    let base_sha = String::from_utf8_lossy(&mb.stdout).trim().to_string();
                    if base_sha == local_sha {
                        return (LocalStatus::Pullable, true, staging_files);
                    }
                }
            }
            if !staging_files.is_empty() {
                (LocalStatus::Staging, true, staging_files)
            } else {
                (LocalStatus::Other, true, vec![])
            }
        }
        (Ok(l), _) if l.status.success() => {
            if !staging_files.is_empty() {
                (LocalStatus::Staging, true, staging_files)
            } else {
                (LocalStatus::Other, true, vec![])
            }
        }
        _ => (LocalStatus::Other, true, vec![]),
    }
}

fn get_staging_files(repo_path: &str) -> Vec<String> {
    let out = Command::new("git")
        .args(["-C", repo_path, "status", "--porcelain"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect()
        }
        _ => vec![],
    }
}

// ──────────────────────────────────────────────
// git pull
// ──────────────────────────────────────────────

pub fn git_pull(base_dir: &str, repo_name: &str) -> Result<String> {
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(['/', '\\']), repo_name);
    let output = Command::new("git")
        .args(["-C", &repo_path, "pull", "--ff-only"])
        .output()
        .context("git pull failed")?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() { Ok(stdout.trim().to_string()) }
    else { bail!("{}", stderr.trim()) }
}

// ──────────────────────────────────────────────
// lazygit
// ──────────────────────────────────────────────

pub fn launch_lazygit(base_dir: &str, repo_name: &str) -> Result<()> {
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(['/', '\\']), repo_name);
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    let status = Command::new("lazygit").args(["-p", &repo_path]).status();
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    );
    match status {
        Ok(_) => Ok(()),
        Err(e) => bail!("lazygit failed: {e}"),
    }
}

// ──────────────────────────────────────────────
// Open URL in browser
// ──────────────────────────────────────────────

pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    { Command::new("cmd").args(["/C", "start", "", url]).spawn().context("Failed to open browser")?; }
    #[cfg(not(target_os = "windows"))]
    { Command::new("xdg-open").arg(url).spawn().context("Failed to open browser")?; }
    Ok(())
}

// ──────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────


/// Check if a newer version of gh-tui is available by comparing
/// crates2.json installed hash vs remote HEAD of the git repo.
/// Returns Some("owner/repo") if update is available, None if up-to-date or not installed.
pub fn check_self_update() -> Option<String> {
    let crates2_path = std::env::var("CARGO_HOME")
        .map(|h| format!("{h}/.crates2.json"))
        .unwrap_or_else(|_| {
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            format!("{home}/.cargo/.crates2.json")
        });

    let content = std::fs::read_to_string(&crates2_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let installs = json.get("installs")?.as_object()?;

    // Find gh-tui entry with git+ URL
    let (installed_hash, owner_repo) = installs.keys().find_map(|key| {
        let mut parts = key.splitn(3, ' ');
        let name = parts.next()?;
        if name != "cat-repo-auditor" { return None; }
        let src = key.trim_end_matches(')');
        let git_prefix = "git+https://github.com/";
        let idx = src.find(git_prefix)?;
        let rest = &src[idx + git_prefix.len()..];
        // rest = "owner/repo#HASH"
        let hash_idx = rest.find('#')?;
        let owner_repo = &rest[..hash_idx];
        let hash = &rest[hash_idx + 1..];
        if hash.is_empty() { return None; }
        Some((hash.to_string(), owner_repo.to_string()))
    })?;

    // Get remote HEAD via gh api
    let endpoint = format!("/repos/{owner_repo}/commits/HEAD");
    let out = Command::new("gh")
        .args(["api", &endpoint, "--jq", ".sha"])
        .output().ok()?;
    if !out.status.success() { return None; }
    let remote_hash = String::from_utf8_lossy(&out.stdout).trim().to_string();

    if remote_hash.is_empty() || remote_hash == installed_hash { return None; }
    Some(owner_repo)
}

fn run_gh_graphql(query: &str) -> Result<String> {
    let output = Command::new("gh")
        .args(["api", "graphql", "-f", &format!("query={}", query)])
        .output()
        .context("Failed to run `gh` command. Is GitHub CLI installed and authenticated?")?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("gh command failed: {err}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn format_date_iso(iso: &str) -> String {
    if let Ok(dt) = iso.parse::<DateTime<Utc>>() {
        dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    } else { iso.to_string() }
}

pub fn relative_date(iso: &str) -> String {
    if let Ok(dt) = iso.parse::<DateTime<Utc>>() {
        let now  = Utc::now();
        let diff = now.signed_duration_since(dt);
        if diff < Duration::days(1)   { String::from("today") }
        else if diff < Duration::weeks(1) { format!("{}d",  diff.num_days()) }
        else if diff < Duration::days(30) { format!("{}w",  diff.num_weeks()) }
        else if diff < Duration::days(365){ format!("{}mo", diff.num_days() / 30) }
        else                              { format!("{}y",  diff.num_days() / 365) }
    } else { iso.to_string() }
}

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
