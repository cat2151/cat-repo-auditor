use crate::{
    config::Config,
    github_fetch::do_fetch,
    github_local::{
        check_cargo_git_install, check_deepwiki_exists, check_file_exists,
        check_pages_exists, check_readme_ja_badge, check_workflows, git_pull,
    },
    history::History,
};
use serde::{Deserialize, Serialize};

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
    /// local git HEAD hash when cargo_install was last checked (doubles as display value for local hash)
    #[serde(default)]
    pub cargo_checked_at: String,
    /// installed commit hash from .crates2.json (only meaningful when cargo_install == Some(false))
    #[serde(default)]
    pub cargo_installed_hash: String,

    /// All 3 required workflow yml files present in .github/workflows/
    #[serde(default)]
    pub wf_workflows: Option<bool>,
    /// local HEAD hash when wf_workflows was last checked
    #[serde(default)]
    pub wf_checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LocalStatus {
    Conflict,
    Modified,
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
            LocalStatus::Conflict => write!(f, "conflict"),
            LocalStatus::Modified => write!(f, "modified"),
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
        cargo_installed_hash:   String,
        wf_workflows:           Option<bool>,
        wf_cat:                 String,
    },
    Done(anyhow::Result<(Vec<RepoInfo>, RateLimit)>),
}

// ──────────────────────────────────────────────
// Fetch orchestration
// ──────────────────────────────────────────────

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
            // Phase 2: auto-pull repos that can be safely fast-forwarded.
            // Dirty repos are handled by stashing before pull and restoring after.
            let pullable: Vec<String> = if config.auto_pull {
                repos.iter()
                    .filter(|r| matches!(
                        r.local_status,
                        LocalStatus::Pullable | LocalStatus::Modified | LocalStatus::Staging
                    ))
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

                let (cargo_install, cargo_cat, cargo_installed_hash) = if needs_cargo {
                    match check_cargo_git_install(&owner, name, &config.local_base_dir) {
                        // Use `loc` (the actual hash read from git) as cargo_cat so the stored
                        // value is always the precise hash used in the comparison.
                        Some((ok, inst, loc)) => (Some(ok), loc, inst),
                        None => (None, local_head.clone(), String::new()),
                    }
                } else {
                    (repo.cargo_install, repo.cargo_checked_at.clone(),
                     repo.cargo_installed_hash.clone())
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
                    r.cargo_installed_hash         = cargo_installed_hash.clone();
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
                    cargo_installed_hash,
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

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
