use crate::{
    config::Config,
    github::{FetchProgress, IssueOrPr, LocalStatus, RateLimit, RepoInfo},
    github_local::{check_local_status_no_fetch, local_head_hash_no_fetch},
    history::History,
};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::process::Command;

// ──────────────────────────────────────────────
// GraphQL response shapes
// ──────────────────────────────────────────────

#[derive(Deserialize)]
struct GqlResponse {
    data: GqlData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlData {
    repository_owner: RepositoryOwner,
    rate_limit: GqlRateLimit,
}

#[derive(Deserialize)]
struct RepositoryOwner {
    repositories: RepositoryConnection,
}

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
    #[serde(rename = "totalCount")]
    total_count: u64,
    nodes: Vec<GqlIssue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlIssue {
    title: String,
    number: u64,
    updated_at: String,
}

#[derive(Deserialize)]
struct PrConnection {
    #[serde(rename = "totalCount")]
    total_count: u64,
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
struct ClosingIssues {
    nodes: Vec<ClosingIssueNode>,
}

#[derive(Deserialize)]
struct ClosingIssueNode {
    number: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GqlRateLimit {
    remaining: u64,
    limit: u64,
    reset_at: String,
}

// ──────────────────────────────────────────────
// Fetch
// ──────────────────────────────────────────────

fn build_repo_info(config: &Config, history: &History, repo: GqlRepo) -> Option<RepoInfo> {
    let GqlRepo {
        name,
        name_with_owner,
        updated_at,
        is_fork,
        is_archived,
        is_private,
        issues,
        pull_requests,
    } = repo;

    if is_fork || is_archived {
        return None;
    }

    let full_name = name_with_owner.clone();
    let history_repo = history.repos.iter().find(|h| h.name == name);
    let (local_status, has_local_git, staging_files, local_head_hash) = history_repo
        .map(|repo| {
            (
                repo.local_status.clone(),
                repo.has_local_git,
                repo.staging_files.clone(),
                repo.local_head_hash.clone(),
            )
        })
        .unwrap_or_else(|| {
            let (local_status, has_local_git, staging_files) =
                check_local_status_no_fetch(&config.local_base_dir, &name);
            let local_head_hash = if has_local_git {
                local_head_hash_no_fetch(&config.local_base_dir, &name)
            } else {
                String::new()
            };
            (local_status, has_local_git, staging_files, local_head_hash)
        });
    let updated_at_raw = format_date_iso(&updated_at);

    let (
        readme_ja,
        readme_ja_checked_at,
        readme_ja_badge,
        readme_ja_badge_checked_at,
        pages,
        pages_checked_at,
        deepwiki,
        deepwiki_checked_at,
        cargo_install,
        cargo_checked_at,
        cargo_remote_hash,
        cargo_remote_hash_checked_at,
        cargo_installed_hash,
        wf_workflows,
        wf_checked_at,
    ) = history_repo
        .map(|h| {
            (
                h.readme_ja,
                h.readme_ja_checked_at.clone(),
                h.readme_ja_badge,
                h.readme_ja_badge_checked_at.clone(),
                h.pages,
                h.pages_checked_at.clone(),
                h.deepwiki,
                h.deepwiki_checked_at.clone(),
                h.cargo_install,
                h.cargo_checked_at.clone(),
                h.cargo_remote_hash.clone(),
                h.cargo_remote_hash_checked_at.clone(),
                h.cargo_installed_hash.clone(),
                h.wf_workflows,
                h.wf_checked_at.clone(),
            )
        })
        .unwrap_or((
            None,
            String::new(),
            None,
            String::new(),
            None,
            String::new(),
            None,
            String::new(),
            None,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            None,
            String::new(),
        ));

    Some(RepoInfo {
        name,
        full_name: full_name.clone(),
        updated_at: relative_date(&updated_at),
        updated_at_raw,
        open_issues: issues.total_count,
        open_prs: pull_requests.total_count,
        is_private,
        local_status,
        has_local_git,
        staging_files,
        local_head_hash,
        readme_ja,
        readme_ja_checked_at,
        readme_ja_badge,
        readme_ja_badge_checked_at,
        pages,
        pages_checked_at,
        deepwiki,
        deepwiki_checked_at,
        cargo_install,
        cargo_checked_at,
        cargo_remote_hash,
        cargo_remote_hash_checked_at,
        cargo_installed_hash,
        wf_workflows,
        wf_checked_at,
        issues: issues
            .nodes
            .into_iter()
            .map(|issue| {
                let raw_issue_updated_at = issue.updated_at.clone();
                IssueOrPr {
                    title: issue.title,
                    number: issue.number,
                    updated_at: relative_date(&raw_issue_updated_at),
                    updated_at_raw: format_date_iso(&raw_issue_updated_at),
                    repo_full: full_name.clone(),
                    is_pr: false,
                    closes_issue: None,
                }
            })
            .collect(),
        prs: pull_requests
            .nodes
            .into_iter()
            .map(|pr| {
                let closes_issue = if pr.closing_issues_references.nodes.len() == 1 {
                    Some(pr.closing_issues_references.nodes[0].number)
                } else {
                    None
                };
                let raw_pr_updated_at = pr.updated_at.clone();
                IssueOrPr {
                    title: pr.title,
                    number: pr.number,
                    updated_at: relative_date(&raw_pr_updated_at),
                    updated_at_raw: format_date_iso(&raw_pr_updated_at),
                    repo_full: full_name.clone(),
                    is_pr: true,
                    closes_issue,
                }
            })
            .collect(),
    })
}

pub(crate) fn do_fetch(
    config: &Config,
    history: &mut History,
    tx: &std::sync::mpsc::Sender<FetchProgress>,
) -> Result<(Vec<RepoInfo>, RateLimit)> {
    let owner = &config.owner;
    let etag_key = format!("repos:{owner}");

    let _ = tx.send(FetchProgress::BeginRepoRefresh(
        history.repos.iter().map(|repo| repo.name.clone()).collect(),
    ));

    let mut repo_infos: Vec<RepoInfo> = vec![];
    let mut cursor: Option<String> = None;
    #[allow(unused_assignments)]
    let mut rate_limit_info: Option<RateLimit> = None;
    let mut page_num = 0u32;
    let mut scan_i = 0usize;

    loop {
        page_num += 1;
        let _ = tx.send(FetchProgress::PhaseProgress {
            tag: "gh↓",
            cur: page_num as usize,
            total: 0,
        });

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
                    history
                        .save(&crate::config::Config::history_path().to_string_lossy())
                        .ok();
                    return Ok((history.repos.clone(), rl_out));
                }
            }
            history.etags.insert(etag_key.clone(), response_hash);
        }

        let page_info = &gql.data.repository_owner.repositories.page_info;
        let has_next = page_info.has_next_page;
        let end_cursor = page_info.end_cursor.clone();
        for repo in gql.data.repository_owner.repositories.nodes {
            if let Some(repo_info) = build_repo_info(config, history, repo) {
                scan_i += 1;
                let _ = tx.send(FetchProgress::PhaseProgress {
                    tag: "scan",
                    cur: scan_i,
                    total: 0,
                });
                let _ = tx.send(FetchProgress::RepoUpdate(repo_info.clone()));
                repo_infos.push(repo_info);
            }
        }
        if has_next {
            cursor = end_cursor;
        } else {
            break;
        }
    }

    fn group_key(r: &RepoInfo) -> u8 {
        if r.is_private {
            return 4;
        }
        if r.local_status == LocalStatus::NotFound {
            return 3;
        }
        if r.open_issues == 0 && r.open_prs == 0 {
            return 2;
        }
        if r.open_prs == 0 {
            return 1;
        }
        0
    }

    repo_infos.sort_by(|a, b| {
        group_key(a)
            .cmp(&group_key(b))
            .then(b.updated_at_raw.cmp(&a.updated_at_raw))
    });

    let rl_out = rate_limit_info.unwrap_or(RateLimit {
        remaining: 0,
        limit: 5000,
        reset_at: String::new(),
    });

    history.repos = repo_infos.clone();
    history.rate_limit = Some(rl_out.clone());
    history
        .save(&crate::config::Config::history_path().to_string_lossy())
        .ok();

    Ok((repo_infos, rl_out))
}

// ──────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────

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

pub(crate) fn format_date_iso(iso: &str) -> String {
    if let Ok(dt) = iso.parse::<DateTime<Utc>>() {
        dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    } else {
        iso.to_string()
    }
}

pub fn relative_date(iso: &str) -> String {
    if let Ok(dt) = iso.parse::<DateTime<Utc>>() {
        let now = Utc::now();
        let diff = now.signed_duration_since(dt);
        if diff < Duration::days(1) {
            String::from("today")
        } else if diff < Duration::weeks(1) {
            format!("{}d", diff.num_days())
        } else if diff < Duration::days(30) {
            format!("{}w", diff.num_weeks())
        } else if diff < Duration::days(365) {
            format!("{}mo", diff.num_days() / 30)
        } else {
            format!("{}y", diff.num_days() / 365)
        }
    } else {
        iso.to_string()
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
#[path = "github_fetch_tests.rs"]
mod tests;
