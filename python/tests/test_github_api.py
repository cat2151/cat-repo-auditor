"""github_api モジュールのテスト。"""
import sys
import os
import json
import subprocess
import urllib.error

import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from cat_repo_auditor.github_api import (
    get_token_from_gh,
    github_request,
    file_exists,
    fetch_dir_listing,
    fetch_root_listing,
)


class _FakeResponse:
    """urllib.request.urlopen のスタブ。"""

    def __init__(self, data):
        self._data = json.dumps(data).encode("utf-8")

    def read(self):
        return self._data

    def __enter__(self):
        return self

    def __exit__(self, *args):
        pass


def _make_urlopen(data):
    def _urlopen(req, timeout=15):
        return _FakeResponse(data)
    return _urlopen


def test_github_request_returns_parsed_json(monkeypatch):
    import urllib.request
    monkeypatch.setattr(urllib.request, "urlopen", _make_urlopen({"key": "value"}))
    result = github_request("https://api.github.com/test", "token")
    assert result == {"key": "value"}


def test_github_request_returns_none_on_404(monkeypatch):
    import urllib.request

    def _raise_404(req, timeout=15):
        raise urllib.error.HTTPError(None, 404, "Not Found", {}, None)

    monkeypatch.setattr(urllib.request, "urlopen", _raise_404)
    result = github_request("https://api.github.com/test", "token")
    assert result is None


def test_file_exists_true_when_dict_returned(monkeypatch):
    import urllib.request
    monkeypatch.setattr(urllib.request, "urlopen", _make_urlopen({"name": "file.txt"}))
    assert file_exists("repo", "file.txt", "token", "user") is True


def test_file_exists_false_when_none_returned(monkeypatch):
    import urllib.request

    def _raise_404(req, timeout=15):
        raise urllib.error.HTTPError(None, 404, "Not Found", {}, None)

    monkeypatch.setattr(urllib.request, "urlopen", _raise_404)
    assert file_exists("repo", "file.txt", "token", "user") is False


def test_fetch_dir_listing_returns_list(monkeypatch):
    import urllib.request
    entries = [{"name": "a.yml", "type": "file"}, {"name": "b.yml", "type": "file"}]
    monkeypatch.setattr(urllib.request, "urlopen", _make_urlopen(entries))
    result = fetch_dir_listing("repo", ".github/workflows", "token", "user")
    assert result == entries


def test_fetch_dir_listing_returns_empty_on_404(monkeypatch):
    import urllib.request

    def _raise_404(req, timeout=15):
        raise urllib.error.HTTPError(None, 404, "Not Found", {}, None)

    monkeypatch.setattr(urllib.request, "urlopen", _raise_404)
    result = fetch_dir_listing("repo", ".github/workflows", "token", "user")
    assert result == []


def test_fetch_root_listing_returns_list(monkeypatch):
    import urllib.request
    entries = [{"name": "README.md", "type": "file"}]
    monkeypatch.setattr(urllib.request, "urlopen", _make_urlopen(entries))
    result = fetch_root_listing("repo", "token", "user")
    assert result == entries


def test_fetch_root_listing_returns_empty_when_not_list(monkeypatch):
    import urllib.request
    monkeypatch.setattr(urllib.request, "urlopen", _make_urlopen({"not": "a list"}))
    result = fetch_root_listing("repo", "token", "user")
    assert result == []


# ---- get_token_from_gh ----

def test_get_token_from_gh_success(monkeypatch):
    completed = subprocess.CompletedProcess(
        args=["gh", "auth", "token"], returncode=0, stdout="ghp_testtoken\n", stderr=""
    )
    monkeypatch.setattr(subprocess, "run", lambda *a, **kw: completed)
    assert get_token_from_gh() == "ghp_testtoken"


def test_get_token_from_gh_not_found(monkeypatch):
    def _raise(cmd, **kw):
        raise FileNotFoundError
    monkeypatch.setattr(subprocess, "run", _raise)
    with pytest.raises(SystemExit):
        get_token_from_gh()


def test_get_token_from_gh_timeout(monkeypatch):
    def _raise(cmd, **kw):
        raise subprocess.TimeoutExpired(cmd, 10)
    monkeypatch.setattr(subprocess, "run", _raise)
    with pytest.raises(SystemExit):
        get_token_from_gh()
