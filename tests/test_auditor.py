from __future__ import annotations

import io
import sys
from pathlib import Path
from typing import Dict, List

import pytest
import requests

PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from cat_repo_auditor.auditor import AuditResult, GitHubClient, audit_user_repositories  # noqa: E402
from cat_repo_auditor.cli import main  # noqa: E402


class StubResponse:
    def __init__(self, status_code: int = 200, json_data=None):
        self.status_code = status_code
        self._json_data = json_data if json_data is not None else {}

    def json(self):
        return self._json_data

    def raise_for_status(self):
        if self.status_code >= 400:
            raise requests.HTTPError(f"status {self.status_code}")


class RecordingSession:
    def __init__(self, responses: List[StubResponse]):
        self._responses = list(responses)
        self.calls: List[Dict] = []

    def get(self, url, params=None, headers=None, timeout=None):
        self.calls.append({"url": url, "params": params or {}, "headers": headers or {}, "timeout": timeout})
        if not self._responses:
            raise AssertionError("Unexpected request")
        return self._responses.pop(0)


def test_list_repositories_uses_headers_and_params():
    session = RecordingSession([StubResponse(200, [{"name": "one", "pushed_at": "2025-01-01T00:00:00Z"}])])
    client = GitHubClient(token="abc123", session=session)

    repos = client.list_repositories("alice", limit=3)

    assert repos == [{"name": "one", "updated_at": "2025-01-01T00:00:00Z"}]
    assert session.calls[0]["params"]["per_page"] == 3
    assert session.calls[0]["headers"]["Authorization"] == "Bearer abc123"


def test_path_exists_handles_missing_and_errors():
    session = RecordingSession([StubResponse(200), StubResponse(404), StubResponse(500)])
    client = GitHubClient(session=session)

    assert client.path_exists("alice", "repo", "README.md") is True
    assert client.path_exists("alice", "repo", "LICENSE") is False
    with pytest.raises(requests.HTTPError):
        client.path_exists("alice", "repo", "CONTRIB")


def test_audit_user_repositories_collects_results():
    class StubClient:
        def __init__(self):
            self.calls = []

        def list_repositories(self, username, limit):
            self.calls.append(("list", username, limit))
            return [{"name": "demo", "updated_at": "2024-01-01T00:00:00Z"}]

        def path_exists(self, username, repo, path):
            self.calls.append(("path", username, repo, path))
            return path == "README.md"

    stub_client = StubClient()
    results = audit_user_repositories("bob", ["README.md", "LICENSE"], client=stub_client, limit=1)

    assert len(results) == 1
    result = results[0]
    assert isinstance(result, AuditResult)
    assert result.found["README.md"] is True
    assert result.found["LICENSE"] is False
    assert result.missing == ["LICENSE"]
    assert ("list", "bob", 1) in stub_client.calls
    assert ("path", "bob", "demo", "LICENSE") in stub_client.calls


def test_cli_outputs_table(tmp_path):
    config_path = tmp_path / "audit_config.toml"
    config_path.write_text(
        'check_items = ["README.md", "LICENSE"]\n\n[display]\nshow_repo_name = true\n',
        encoding="utf-8",
    )

    class StubClient(GitHubClient):
        def __init__(self):
            super().__init__(token="token")

        def list_repositories(self, username, limit):
            return [{"name": "sample", "updated_at": "2024-02-01"}]

        def path_exists(self, username, repo, path):
            return path == "README.md"

    buffer = io.StringIO()
    exit_code = main(
        ["--user", "alice", "--config", str(config_path), "--limit", "1"],
        client=StubClient(),
        stream=buffer,
    )

    output = buffer.getvalue()
    assert exit_code == 0
    assert "sample" in output
    assert "README.md" in output and "yes" in output
    assert "LICENSE" in output and "no" in output
