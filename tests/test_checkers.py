"""checkers モジュールのテスト。"""
import sys
import os
import base64
import json
import urllib.error

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from cat_repo_auditor.checkers import (
    check_deepwiki,
    analyze_readme,
    check_google_html,
    check_agents_file,
    check_workflows,
    check_jekyll_config,
    fetch_readme_ja,
)


# ---- check_deepwiki ----

def test_check_deepwiki_detects_pattern():
    content = "See https://deepwiki.com/badge.svg for details"
    result = check_deepwiki(content)
    assert result["has_deepwiki"] is True
    assert "deepwiki.com" in result["matched_patterns"]


def test_check_deepwiki_no_match():
    result = check_deepwiki("No deepwiki here")
    # 'deepwiki' is a substring of DEEPWIKI_PATTERNS, so this will match
    # Let's use content without any pattern
    result = check_deepwiki("No relevant content here at all")
    assert result["has_deepwiki"] is False
    assert result["matched_patterns"] == []
    assert result["occurrences"] == []


def test_check_deepwiki_records_occurrences():
    content = "line1\ndeepwiki.com link here\nline3"
    result = check_deepwiki(content)
    assert len(result["occurrences"]) == 1
    assert result["occurrences"][0]["line"] == 2


# ---- analyze_readme ----

def test_analyze_readme_counts_chars():
    content = "hello"
    result = analyze_readme(content)
    assert result["char_count"] == 5


def test_analyze_readme_counts_lines():
    content = "line1\nline2\nline3"
    result = analyze_readme(content)
    assert result["line_count"] == 3


def test_analyze_readme_counts_headings():
    content = "# Heading 1\nsome text\n## Heading 2"
    result = analyze_readme(content)
    assert result["heading_count"] == 2
    assert "# Heading 1" in result["headings"]


def test_analyze_readme_counts_urls():
    content = "Visit https://example.com and https://github.com"
    result = analyze_readme(content)
    assert result["url_count"] == 2


def test_analyze_readme_deduplicates_urls():
    content = "https://example.com and https://example.com again"
    result = analyze_readme(content)
    assert result["url_count"] == 1


# ---- check_google_html ----

def test_check_google_html_found():
    root_files = [
        {"name": "google123abc.html", "type": "file"},
        {"name": "README.md", "type": "file"},
    ]
    result = check_google_html(root_files)
    assert result["exists"] is True
    assert "google123abc.html" in result["files"]


def test_check_google_html_not_found():
    root_files = [{"name": "README.md", "type": "file"}]
    result = check_google_html(root_files)
    assert result["exists"] is False
    assert result["files"] == []


# ---- check_agents_file ----

def test_check_agents_file_found_in_root(monkeypatch):
    import urllib.request

    def _raise_404(req, timeout=15):
        raise urllib.error.HTTPError(None, 404, "Not Found", {}, None)

    monkeypatch.setattr(urllib.request, "urlopen", _raise_404)
    root_files = [{"name": "AGENTS.md"}, {"name": "README.md"}]
    result = check_agents_file("repo", root_files, "token", "user")
    assert result["exists"] is True
    assert "AGENTS.md" in result["found_files"]


def test_check_agents_file_not_found(monkeypatch):
    import urllib.request

    def _raise_404(req, timeout=15):
        raise urllib.error.HTTPError(None, 404, "Not Found", {}, None)

    monkeypatch.setattr(urllib.request, "urlopen", _raise_404)
    root_files = [{"name": "README.md"}]
    result = check_agents_file("repo", root_files, "token", "user")
    assert result["exists"] is False


# ---- check_workflows ----

def test_check_workflows_found(monkeypatch):
    import urllib.request

    class _FakeResp:
        def read(self):
            return json.dumps([
                {"name": "ci.yml", "type": "file"},
                {"name": "deploy.yaml", "type": "file"},
            ]).encode()
        def __enter__(self): return self
        def __exit__(self, *a): pass

    monkeypatch.setattr(urllib.request, "urlopen", lambda req, timeout=15: _FakeResp())
    result = check_workflows("repo", "token", "user")
    assert result["exists"] is True
    assert "ci.yml" in result["files"]


def test_check_workflows_not_found(monkeypatch):
    import urllib.request

    def _raise_404(req, timeout=15):
        raise urllib.error.HTTPError(None, 404, "Not Found", {}, None)

    monkeypatch.setattr(urllib.request, "urlopen", _raise_404)
    result = check_workflows("repo", "token", "user")
    assert result["exists"] is False


# ---- check_jekyll_config ----

def test_check_jekyll_config_found():
    root_files = [{"name": "_config.yml", "type": "file"}]
    result = check_jekyll_config(root_files)
    assert result["exists"] is True


def test_check_jekyll_config_not_found():
    root_files = [{"name": "README.md", "type": "file"}]
    result = check_jekyll_config(root_files)
    assert result["exists"] is False


# ---- fetch_readme_ja ----

def test_fetch_readme_ja_returns_content(monkeypatch):
    import urllib.request
    content = "# README"
    encoded = base64.b64encode(content.encode()).decode()

    class _FakeResp:
        def read(self):
            return json.dumps({"content": encoded}).encode()
        def __enter__(self): return self
        def __exit__(self, *a): pass

    monkeypatch.setattr(urllib.request, "urlopen", lambda req, timeout=15: _FakeResp())
    result = fetch_readme_ja("repo", "token", "user")
    assert result == content


def test_fetch_readme_ja_returns_none_on_404(monkeypatch):
    import urllib.request

    def _raise_404(req, timeout=15):
        raise urllib.error.HTTPError(None, 404, "Not Found", {}, None)

    monkeypatch.setattr(urllib.request, "urlopen", _raise_404)
    result = fetch_readme_ja("repo", "token", "user")
    assert result is None
