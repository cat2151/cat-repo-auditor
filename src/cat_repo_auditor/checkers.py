"""
各リポジトリに対するチェック関数群。
"""

import re
import base64
import time
from fnmatch import fnmatch

from .constants import DEEPWIKI_PATTERNS
from .github_api import github_request, file_exists, fetch_dir_listing


def fetch_readme_ja(repo_name: str, token: str, github_user: str) -> str | None:
    """GitHub から README.ja.md の内容を取得して返す。存在しない場合は None。"""
    url = (
        f"https://api.github.com/repos/{github_user}/{repo_name}"
        f"/contents/README.ja.md?ref=main"
    )
    data = github_request(url, token)
    time.sleep(0.2)
    if not data or "content" not in data:
        return None
    try:
        return base64.b64decode(data["content"]).decode("utf-8")
    except Exception:
        return None


def check_deepwiki(content: str) -> dict:
    """README 内容から DeepWiki 記載を検出する。"""
    found_patterns = []
    occurrences = []
    for i, line in enumerate(content.splitlines(), 1):
        for pattern in DEEPWIKI_PATTERNS:
            if pattern in line:
                if pattern not in found_patterns:
                    found_patterns.append(pattern)
                occurrences.append({"line": i, "text": line.strip()})
                break
    return {
        "has_deepwiki": bool(found_patterns),
        "matched_patterns": found_patterns,
        "occurrences": occurrences,
    }


def analyze_readme(content: str) -> dict:
    """README の文字数・行数・見出し・URL 数を分析する。"""
    lines = content.splitlines()
    non_empty = [l for l in lines if l.strip()]
    headings = [l.strip() for l in lines if l.startswith("#")]
    urls = re.findall(r'https?://[^\s\)\]\"\']+', content)
    return {
        "char_count": len(content),
        "line_count": len(lines),
        "non_empty_lines": len(non_empty),
        "heading_count": len(headings),
        "headings": headings[:10],
        "url_count": len(set(urls)),
    }


def check_google_html(root_files: list) -> dict:
    """ルートに google*.html が存在するか確認する。"""
    names = [f["name"] for f in root_files if f.get("type") == "file"]
    matched = [n for n in names if fnmatch(n.lower(), "google*.html")]
    return {"exists": bool(matched), "files": matched}


def check_agents_file(repo_name: str, root_files: list, token: str, github_user: str) -> dict:
    """AGENTS.md または copilot-instructions.md の存在を確認する。"""
    root_names = {f["name"] for f in root_files}
    found = []
    if "AGENTS.md" in root_names:
        found.append("AGENTS.md")
    if "copilot-instructions.md" in root_names:
        found.append("copilot-instructions.md")
    if file_exists(repo_name, ".github/copilot-instructions.md", token, github_user):
        found.append(".github/copilot-instructions.md")
    return {"exists": bool(found), "found_files": found}


def check_workflows(repo_name: str, token: str, github_user: str) -> dict:
    """`.github/workflows/` 配下に yml/yaml が存在するか確認する。"""
    entries = fetch_dir_listing(repo_name, ".github/workflows", token, github_user)
    ymls = [
        e["name"] for e in entries
        if e.get("type") == "file"
        and (e["name"].endswith(".yml") or e["name"].endswith(".yaml"))
    ]
    return {"exists": bool(ymls), "files": ymls}


def check_jekyll_config(root_files: list) -> dict:
    """ルートに _config.yml (Jekyll設定) が存在するか確認する。"""
    names = {f["name"] for f in root_files if f.get("type") == "file"}
    return {"exists": "_config.yml" in names}
