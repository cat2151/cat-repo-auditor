"""Lightweight GitHub repository auditing helpers."""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Dict, List, Sequence

import requests

API_BASE = "https://api.github.com"
DEFAULT_REPO_LIMIT = 20


@dataclass
class AuditResult:
    """Result for a single repository."""

    repository: str
    updated_at: str | None
    found: Dict[str, bool]

    @property
    def missing(self) -> List[str]:
        """Return a list of items that were not found."""
        return [item for item, present in self.found.items() if not present]


class GitHubClient:
    """Small wrapper around requests for the GitHub REST API."""

    def __init__(self, token: str | None = None, session: requests.Session | None = None) -> None:
        self.session = session or requests.Session()
        self.token = token or os.getenv("GITHUB_TOKEN")

    def _headers(self) -> Dict[str, str]:
        headers = {
            "Accept": "application/vnd.github.v3+json",
            "User-Agent": "cat-repo-auditor/0.1",
        }
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        return headers

    def list_repositories(self, username: str, limit: int = DEFAULT_REPO_LIMIT) -> List[Dict[str, str | None]]:
        """
        Retrieve repositories for a GitHub user ordered by recent activity.

        Args:
            username: GitHub username.
            limit: Maximum number of repositories to return.

        Returns:
            Basic repository metadata.

        Raises:
            HTTPError: When the GitHub API responds with an error.
        """
        response = self.session.get(
            f"{API_BASE}/users/{username}/repos",
            params={"per_page": limit, "sort": "pushed"},
            headers=self._headers(),
            timeout=10,
        )
        response.raise_for_status()
        repos = response.json()
        return [{"name": repo.get("name", ""), "updated_at": repo.get("pushed_at")} for repo in repos]

    def path_exists(self, username: str, repo: str, path: str) -> bool:
        """
        Check if a path exists in a repository.

        Args:
            username: Repository owner.
            repo: Repository name.
            path: Path to check.

        Returns:
            True when the path exists, False when missing.

        Raises:
            HTTPError: For non-404 error responses.
        """
        response = self.session.get(
            f"{API_BASE}/repos/{username}/{repo}/contents/{path}",
            headers=self._headers(),
            timeout=10,
        )
        if response.status_code == 200:
            return True
        if response.status_code == 404:
            return False
        response.raise_for_status()
        return False


def audit_user_repositories(
    username: str,
    check_items: Sequence[str],
    *,
    limit: int = DEFAULT_REPO_LIMIT,
    client: GitHubClient | None = None,
    token: str | None = None,
) -> List[AuditResult]:
    """
    Audit repositories for required paths.

    Args:
        username: GitHub username to inspect.
        check_items: Paths to verify.
        limit: Maximum repositories to fetch.
        client: Optional GitHubClient override (useful for testing).
        token: Personal access token, falls back to GITHUB_TOKEN env var.

    Returns:
        List of AuditResult objects.
    """
    auditor = client or GitHubClient(token=token)
    repos = auditor.list_repositories(username, limit)
    results: List[AuditResult] = []

    for repo in repos:
        found = {item: auditor.path_exists(username, repo["name"], item) for item in check_items}
        results.append(AuditResult(repository=repo["name"], updated_at=repo.get("updated_at"), found=found))

    return results
