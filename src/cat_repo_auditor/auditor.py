"""Lightweight GitHub repository auditing helpers."""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from typing import Dict, List, Sequence, TypedDict

import requests
from urllib.parse import quote

API_BASE = "https://api.github.com"
DEFAULT_REPO_LIMIT = 20


class RepositoryInfo(TypedDict):
    """Basic repository metadata."""

    name: str
    updated_at: str | None


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
        self._username_pattern = re.compile(r"^[A-Za-z0-9-]+$")

    def _headers(self) -> Dict[str, str]:
        headers = {
            "Accept": "application/vnd.github.v3+json",
            "User-Agent": "cat-repo-auditor/0.1",
        }
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        return headers

    def list_repositories(self, username: str, limit: int = DEFAULT_REPO_LIMIT) -> List[RepositoryInfo]:
        """
        Retrieve repositories for a GitHub user ordered by recent activity.

        Args:
            username: GitHub username.
            limit: Maximum number of repositories to return.

        Returns:
            Basic repository metadata.

        Raises:
            requests.HTTPError: When the GitHub API responds with an error.
            ValueError: If the username is invalid or response is unexpected.
        """
        if not self._username_pattern.match(username):
            raise ValueError("Username must be alphanumeric and may include hyphens.")

        response = self.session.get(
            f"{API_BASE}/users/{username}/repos",
            params={"per_page": limit, "sort": "pushed"},
            headers=self._headers(),
            timeout=10,
        )
        response.raise_for_status()
        repos = response.json()
        if not isinstance(repos, list):
            raise ValueError("Unexpected response when listing repositories.")

        results: List[RepositoryInfo] = []
        for repo in repos:
            name = repo.get("name")
            if not isinstance(name, str):
                continue
            results.append({"name": name, "updated_at": repo.get("pushed_at")})
        return results

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
            requests.HTTPError: For non-404 error responses.
            ValueError: If inputs are invalid.
        """
        if not self._username_pattern.match(username):
            raise ValueError("Username must be alphanumeric and may include hyphens.")
        if not repo or "/" in repo:
            raise ValueError("Repository name must be a single path segment.")
        if not path:
            raise ValueError("Path must be non-empty.")

        response = self.session.get(
            f"{API_BASE}/repos/{quote(username)}/{quote(repo)}/contents/{quote(path)}",
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

    Note:
        This makes a request for every repository-path combination, which can be slow
        and may hit rate limits for large inputs.
    """
    auditor = client or GitHubClient(token=token)
    repos = auditor.list_repositories(username, limit)
    results: List[AuditResult] = []

    for repo in repos:
        found = {item: auditor.path_exists(username, repo["name"], item) for item in check_items}
        results.append(AuditResult(repository=repo["name"], updated_at=repo.get("updated_at"), found=found))

    return results
