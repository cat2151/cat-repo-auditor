use serde::{Deserialize, Serialize};

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
        format!(
            "https://github.com/{}/{}/{}",
            self.repo_full, kind, self.number
        )
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
    #[serde(default)]
    pub local_head_hash: String,
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
    /// Some(true) = installed hash == remote main HEAD, Some(false) = stale against upstream
    #[serde(default)]
    pub cargo_install: Option<bool>,
    /// local git HEAD hash when cargo_install was last checked (cargo cache only)
    #[serde(default)]
    pub cargo_checked_at: String,
    /// remote main branch HEAD hash from GitHub (used for cargo hash display/comparison)
    #[serde(default)]
    pub cargo_remote_hash: String,
    /// updated_at_raw when cargo_remote_hash was last checked
    #[serde(default)]
    pub cargo_remote_hash_checked_at: String,
    /// installed commit hash from .crates2.json
    #[serde(default)]
    pub cargo_installed_hash: String,

    /// Required workflow yml files are present in .github/workflows/
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
            LocalStatus::Clean => write!(f, "clean"),
            LocalStatus::Staging => write!(f, "staging"),
            LocalStatus::Other => write!(f, "other"),
            LocalStatus::NotFound => write!(f, "-"),
            LocalStatus::NoGit => write!(f, "no-git"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub remaining: u64,
    pub limit: u64,
    pub reset_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoUpdateLaunchRequest {
    pub name: String,
    pub full_name: String,
    pub cargo_install: Option<bool>,
    pub installed_hash: String,
    pub remote_hash: String,
}

pub enum FetchProgress {
    Log(String),
    BackgroundChecksCompleted,
    /// Reset issue/PR pending state for the next repo refresh pass.
    BeginRepoRefresh(Vec<String>),
    /// Reset local-status pending state for repos whose local check is about to run.
    BeginLocalRefresh(Vec<String>),
    /// Reset cargo pending state for repos whose cargo check is about to run.
    BeginCargoRefresh(Vec<String>),
    /// Structured progress for background task display: (tag, cur, total)
    /// tag examples: "gh↓", "scan", "pull", "lcl", "chk"
    PhaseProgress {
        tag: &'static str,
        cur: usize,
        total: usize,
    },
    /// Incremental repo snapshot from GitHub fetch so the UI can clear issue/PR spinners per repo.
    RepoUpdate(Box<RepoInfo>),
    /// Signal that a specific repo is currently being checked (for UI feedback)
    CheckingRepo(String),
    /// Incremental update per repo after phase-3 checks
    ExistenceUpdate {
        name: String,
        local_status: LocalStatus,
        has_local_git: bool,
        staging_files: Vec<String>,
        local_head_hash: String,
        readme_ja: Option<bool>,
        readme_ja_cat: String,
        readme_ja_badge: Option<bool>,
        readme_ja_badge_cat: String,
        pages: Option<bool>,
        pages_cat: String,
        deepwiki: Option<bool>,
        deepwiki_cat: String,
        wf_workflows: Option<bool>,
        wf_cat: String,
    },
    /// Incremental cargo-only update that can arrive independently of other checks.
    CargoUpdate {
        name: String,
        cargo_install: Option<bool>,
        cargo_cat: String,
        cargo_remote_hash: String,
        cargo_remote_hash_cat: String,
        cargo_installed_hash: String,
    },
    RequestAutoUpdateLaunch(AutoUpdateLaunchRequest),
    Done(anyhow::Result<(Vec<RepoInfo>, RateLimit)>),
}
