use crate::{
    config::Config,
    github_fetch::do_fetch,
    github_local::{
        append_cargo_check_results, check_cargo_git_install, check_deepwiki_exists,
        check_file_exists, check_pages_exists, check_readme_ja_badge, check_workflows, git_pull,
        local_head_matches_upstream,
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
    /// remote main branch HEAD hash from GitHub (used for cargo hash display/comparison)
    #[serde(default)]
    pub cargo_remote_hash: String,
    /// updated_at_raw when cargo_remote_hash was last checked
    #[serde(default)]
    pub cargo_remote_hash_checked_at: String,
    /// installed commit hash from .crates2.json
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

// ──────────────────────────────────────────────
// Progress channel
// ──────────────────────────────────────────────

pub enum FetchProgress {
    Status(String),
    Log(String),
    BackgroundChecksCompleted,
    /// Structured progress for background task display: (tag, cur, total)
    /// tag examples: "gh↓", "scan", "pull", "chk"
    PhaseProgress {
        tag: &'static str,
        cur: usize,
        total: usize,
    },
    /// Signal that a specific repo is currently being checked (for UI feedback)
    CheckingRepo(String),
    /// Incremental update per repo after phase-3 checks
    ExistenceUpdate {
        name: String,
        readme_ja: Option<bool>,
        readme_ja_cat: String,
        readme_ja_badge: Option<bool>,
        readme_ja_badge_cat: String,
        pages: Option<bool>,
        pages_cat: String,
        deepwiki: Option<bool>,
        deepwiki_cat: String,
        cargo_install: Option<bool>,
        cargo_cat: String,
        cargo_remote_hash: String,
        cargo_remote_hash_cat: String,
        cargo_installed_hash: String,
        wf_workflows: Option<bool>,
        wf_cat: String,
    },
    Done(anyhow::Result<(Vec<RepoInfo>, RateLimit)>),
}

type PullTarget = (String, String);

fn should_auto_pull_status(local_status: &LocalStatus, head_matches_upstream: bool) -> bool {
    match local_status {
        LocalStatus::Pullable => true,
        LocalStatus::Modified | LocalStatus::Staging => !head_matches_upstream,
        _ => false,
    }
}

fn should_auto_pull_repo(base_dir: &str, repo: &RepoInfo) -> bool {
    let head_matches_upstream = matches!(
        repo.local_status,
        LocalStatus::Modified | LocalStatus::Staging
    ) && local_head_matches_upstream(base_dir, &repo.name);
    should_auto_pull_status(&repo.local_status, head_matches_upstream)
}

fn compact_log_detail(detail: &str) -> String {
    detail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn format_pull_log(repo_full_name: &str, pull_result: &anyhow::Result<String>) -> String {
    match pull_result {
        Ok(output) => {
            let detail = compact_log_detail(output);
            if detail.is_empty() {
                format!("pull {repo_full_name}: ok")
            } else {
                format!("pull {repo_full_name}: {detail}")
            }
        }
        Err(err) => format!(
            "pull {repo_full_name} failed: {}",
            compact_log_detail(&format!("{err:#}"))
        ),
    }
}

/// Cargo check の状態とログ用の説明材料を保持する。
///
/// `needs_local` / `needs_remote` は実行判定には使わず、ログで
/// 「何が最新か / 何が古いか」を説明するために保持している。
#[derive(Clone, Copy)]
struct CargoCheckStatus {
    needs_local: bool,
    needs_remote: bool,
}

impl CargoCheckStatus {
    fn for_repo(repo: &RepoInfo, local_head: &str) -> Self {
        Self {
            needs_local: repo.cargo_checked_at != local_head,
            needs_remote: repo.cargo_remote_hash_checked_at != repo.updated_at_raw
                || repo.cargo_remote_hash.is_empty(),
        }
    }
}

fn cargo_check_status(
    cargo_check_statuses: &std::collections::HashMap<String, CargoCheckStatus>,
    repo_name: &str,
) -> CargoCheckStatus {
    cargo_check_statuses
        .get(repo_name)
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "repo '{repo_name}' のcargo状態が見つかりません。すべてのrepoに状態が存在する想定です"
            )
        })
}

fn format_cargo_check_status_reason(status: CargoCheckStatus) -> &'static str {
    match (status.needs_local, status.needs_remote) {
        (false, false) => {
            "cargo check を実行: local HEAD と remote hash cache は最新ですが、installed hash 確認のため毎回実行します"
        }
        (false, true) => {
            "cargo check を実行: local HEAD cache は最新ですが、remote hash cache が古いか空です"
        }
        (true, false) => {
            "cargo check を実行: remote hash cache は最新ですが、local HEAD cache が古いです"
        }
        (true, true) => {
            "cargo check を実行: local HEAD cache と remote hash cache の両方が古いか空です"
        }
    }
}

fn format_cargo_check_status_log(
    repo: &RepoInfo,
    local_head: &str,
    status: CargoCheckStatus,
) -> String {
    format!(
        "{}: needs_cargo_local={} needs_cargo_remote={} local_head={:?} cargo_checked_at={:?} updated_at_raw={:?} cargo_remote_hash_checked_at={:?} cargo_remote_hash_present={} cargo_install={:?}",
        format_cargo_check_status_reason(status),
        status.needs_local,
        status.needs_remote,
        local_head,
        repo.cargo_checked_at,
        repo.updated_at_raw,
        repo.cargo_remote_hash_checked_at,
        !repo.cargo_remote_hash.is_empty(),
        repo.cargo_install,
    )
}

fn resolve_cargo_check_fields(
    repo: &RepoInfo,
    updated_at_raw: &str,
    cargo_result: Option<(bool, String, String, String)>,
) -> (Option<bool>, String, String, String, String) {
    match cargo_result {
        // `loc`（git から実際に読んだ hash）を cargo_cat に使い、
        // 保存値が常に比較に使った正確な hash になるようにする。
        Some((ok, inst, loc, remote)) => (Some(ok), loc, remote, updated_at_raw.to_string(), inst),
        None => (
            repo.cargo_install,
            repo.cargo_checked_at.clone(),
            repo.cargo_remote_hash.clone(),
            repo.cargo_remote_hash_checked_at.clone(),
            repo.cargo_installed_hash.clone(),
        ),
    }
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
        Err(e) => {
            let _ = tx.send(FetchProgress::Done(Err(e)));
        }
        Ok((mut repos, rl)) => {
            // Phase 2: auto-pull repos that can be safely fast-forwarded.
            // Dirty repos are handled by stashing before pull and restoring after.
            let pullable: Vec<PullTarget> = if config.auto_pull {
                repos
                    .iter()
                    .filter(|r| should_auto_pull_repo(&config.local_base_dir, r))
                    .map(|r| (r.name.clone(), r.full_name.clone()))
                    .collect()
            } else {
                vec![]
            };
            if !pullable.is_empty() {
                let total = pullable.len();
                for (i, (name, repo_full_name)) in pullable.iter().enumerate() {
                    let _ = tx.send(FetchProgress::PhaseProgress {
                        tag: "pull",
                        cur: i + 1,
                        total,
                    });
                    let pull_result = git_pull(&config.local_base_dir, name);
                    let _ = tx.send(FetchProgress::Log(format_pull_log(
                        repo_full_name,
                        &pull_result,
                    )));
                }
                let _ = tx.send(FetchProgress::Status(String::from(
                    "Refreshing after auto-pull…",
                )));
                match do_fetch(&config, &mut history, &tx) {
                    Ok((r2, rl2)) => {
                        repos = r2;
                        let _ = tx.send(FetchProgress::Done(Ok((repos.clone(), rl2))));
                    }
                    Err(e) => {
                        let _ = tx.send(FetchProgress::Done(Err(e)));
                        return;
                    }
                }
            } else {
                let _ = tx.send(FetchProgress::Done(Ok((repos.clone(), rl))));
            }

            // Phase 3:
            // - README / Pages / DeepWiki / workflows は各 checked_at が古いときだけ再確認する。
            // - cargo install 状態の確認は毎回実行し、cargo_checked_at /
            //   cargo_remote_hash_checked_at はその結果表示用の記録として更新する。
            let owner = config.owner.clone();

            // Collect local HEAD hashes once (cheap, no network)
            let local_heads: std::collections::HashMap<String, String> = repos
                .iter()
                .filter(|r| r.has_local_git)
                .filter_map(|r| {
                    let path = format!(
                        "{}/{}",
                        config.local_base_dir.trim_end_matches(['/', '\\']),
                        r.name
                    );
                    let out = std::process::Command::new("git")
                        .args(["-C", &path, "rev-parse", "HEAD"])
                        .output()
                        .ok()?;
                    if !out.status.success() {
                        return None;
                    }
                    Some((
                        r.name.clone(),
                        String::from_utf8_lossy(&out.stdout).trim().to_string(),
                    ))
                })
                .collect();

            let cargo_check_statuses: std::collections::HashMap<String, CargoCheckStatus> = repos
                .iter()
                .map(|repo| {
                    let local_head = local_heads
                        .get(&repo.name)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    (
                        repo.name.clone(),
                        CargoCheckStatus::for_repo(repo, local_head),
                    )
                })
                .collect();

            // Build per-repo check tasks.
            // cargo install 状態の確認は毎回実行するため、Phase 3 は全 repo を対象にする。
            let to_check: Vec<String> = repos.iter().map(|r| r.name.clone()).collect();

            let cargo_check_logs: Vec<(String, String)> = repos
                .iter()
                .map(|repo| {
                    let local_head = local_heads
                        .get(&repo.name)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    let status = cargo_check_status(&cargo_check_statuses, &repo.name);
                    (
                        repo.name.clone(),
                        format_cargo_check_status_log(repo, local_head, status),
                    )
                })
                .collect();
            append_cargo_check_results(&owner, &cargo_check_logs);

            if to_check.is_empty() {
                return;
            }
            let total_check = to_check.len();

            for (i, name) in to_check.iter().enumerate() {
                let repo = repos.iter().find(|r| &r.name == name).unwrap();
                let cat = repo.updated_at_raw.clone();
                let local_head = local_heads.get(name).cloned().unwrap_or_default();

                let needs_readme = repo.readme_ja_checked_at != cat;
                let needs_ja_badge = repo.readme_ja_badge_checked_at != local_head;
                let needs_pages = repo.pages_checked_at != cat;
                let needs_deepwiki = repo.deepwiki_checked_at != local_head;
                let needs_wf = repo.wf_checked_at != local_head;

                // Signal UI that this repo is being checked
                let _ = tx.send(FetchProgress::CheckingRepo(name.clone()));
                let _ = tx.send(FetchProgress::PhaseProgress {
                    tag: "chk",
                    cur: i + 1,
                    total: total_check,
                });

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
                    (
                        repo.readme_ja_badge,
                        repo.readme_ja_badge_checked_at.clone(),
                    )
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

                let (
                    cargo_install,
                    cargo_cat,
                    cargo_remote_hash,
                    cargo_remote_hash_cat,
                    cargo_installed_hash,
                ) = resolve_cargo_check_fields(
                    repo,
                    &cat,
                    check_cargo_git_install(&owner, name, &config.local_base_dir),
                );

                let (wf_workflows, wf_cat) = if needs_wf {
                    let v = check_workflows(&config.local_base_dir, name);
                    (Some(v), local_head.clone())
                } else {
                    (repo.wf_workflows, repo.wf_checked_at.clone())
                };

                if let Some(r) = history.repos.iter_mut().find(|r| &r.name == name) {
                    r.readme_ja = readme_ja;
                    r.readme_ja_checked_at = readme_ja_cat.clone();
                    r.readme_ja_badge = readme_ja_badge;
                    r.readme_ja_badge_checked_at = readme_ja_badge_cat.clone();
                    r.pages = pages;
                    r.pages_checked_at = pages_cat.clone();
                    r.deepwiki = deepwiki;
                    r.deepwiki_checked_at = deepwiki_cat.clone();
                    r.cargo_install = cargo_install;
                    r.cargo_checked_at = cargo_cat.clone();
                    r.cargo_remote_hash = cargo_remote_hash.clone();
                    r.cargo_remote_hash_checked_at = cargo_remote_hash_cat.clone();
                    r.cargo_installed_hash = cargo_installed_hash.clone();
                    r.wf_workflows = wf_workflows;
                    r.wf_checked_at = wf_cat.clone();
                }

                let _ = tx.send(FetchProgress::ExistenceUpdate {
                    name: name.clone(),
                    readme_ja,
                    readme_ja_cat,
                    readme_ja_badge,
                    readme_ja_badge_cat,
                    pages,
                    pages_cat,
                    deepwiki,
                    deepwiki_cat,
                    cargo_install,
                    cargo_cat,
                    cargo_remote_hash,
                    cargo_remote_hash_cat,
                    cargo_installed_hash,
                    wf_workflows,
                    wf_cat,
                });
            }
            let _ = tx.send(FetchProgress::BackgroundChecksCompleted);
            // Clear progress indicators
            let _ = tx.send(FetchProgress::CheckingRepo(String::new()));
            let _ = tx.send(FetchProgress::PhaseProgress {
                tag: "chk",
                cur: 0,
                total: 0,
            });
            history
                .save(&crate::config::Config::history_path().to_string_lossy())
                .ok();
        }
    }
}

#[cfg(test)]
#[path = "github_tests.rs"]
mod tests;
