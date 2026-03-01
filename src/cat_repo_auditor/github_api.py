"""
GitHub API クライアント。gh コマンドによるトークン取得と API リクエストを提供する。
"""

import json
import sys
import subprocess
import time
import urllib.request
import urllib.error

from .colors import C


def get_token_from_gh() -> str:
    """gh auth token コマンドで GitHub トークンを取得する。"""
    try:
        result = subprocess.run(
            ["gh", "auth", "token"],
            capture_output=True, text=True, timeout=10
        )
        token = result.stdout.strip()
        if not token:
            print(f"{C.NG_RED}ERROR{C.RESET}: `gh auth token` がトークンを返さなかった。", file=sys.stderr)
            print("  `gh auth login` で認証してから再実行してくれ。", file=sys.stderr)
            sys.exit(1)
        return token
    except FileNotFoundError:
        print(f"{C.NG_RED}ERROR{C.RESET}: gh コマンドが見つからない。GitHub CLI をインストールしてくれ。", file=sys.stderr)
        sys.exit(1)
    except subprocess.TimeoutExpired:
        print(f"{C.NG_RED}ERROR{C.RESET}: `gh auth token` がタイムアウトした。", file=sys.stderr)
        sys.exit(1)


def github_request(url: str, token: str) -> dict | list | None:
    """GitHub API にリクエストし、レスポンスを返す。"""
    req = urllib.request.Request(url)
    req.add_header("Accept", "application/vnd.github+json")
    req.add_header("X-GitHub-Api-Version", "2022-11-28")
    req.add_header("User-Agent", "github-repo-analyzer/1.0")
    req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise
    except Exception as e:
        print(f"  [ERROR] リクエスト失敗: {url} -> {e}", file=sys.stderr)
        return None


def file_exists(repo_name: str, path: str, token: str, github_user: str) -> bool:
    """GitHub 上のファイルが存在するか確認する。"""
    url = f"https://api.github.com/repos/{github_user}/{repo_name}/contents/{path}?ref=main"
    data = github_request(url, token)
    time.sleep(0.2)
    return data is not None and isinstance(data, dict)


def fetch_dir_listing(repo_name: str, path: str, token: str, github_user: str) -> list:
    """GitHub 上のディレクトリ一覧を取得する。"""
    url = f"https://api.github.com/repos/{github_user}/{repo_name}/contents/{path}?ref=main"
    data = github_request(url, token)
    time.sleep(0.2)
    if data is None or not isinstance(data, list):
        return []
    return data


def fetch_root_listing(repo_name: str, token: str, github_user: str) -> list:
    """GitHub 上のルートディレクトリ一覧を取得する。"""
    url = f"https://api.github.com/repos/{github_user}/{repo_name}/contents/?ref=main"
    data = github_request(url, token)
    time.sleep(0.2)
    if data is None or not isinstance(data, list):
        return []
    return data
