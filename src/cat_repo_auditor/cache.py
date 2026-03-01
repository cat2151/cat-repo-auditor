"""
キャッシュ・履歴・リポジトリ設定ファイルの読み書き。
"""

import json
from datetime import datetime
from pathlib import Path

from .colors import C, ok, dim, repo
from .config_loader import tomllib
from .constants import CACHE_DIR, HISTORY_FILE, REPO_CACHE_FILE, CONFIG_DIR, REPO_CONFIG_FILE


def _ensure_dir(path: Path) -> None:
    """ディレクトリが存在しない場合は作成する。"""
    path.mkdir(parents=True, exist_ok=True)


def load_history() -> dict:
    """cache/history.json を読み込む。存在しない場合は空dictを返す。"""
    if not HISTORY_FILE.exists():
        return {}
    try:
        data = json.loads(HISTORY_FILE.read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            return {}
        return data
    except (json.JSONDecodeError, OSError):
        return {}


def save_history() -> None:
    """現在日時を cache/history.json に保存する。"""
    _ensure_dir(CACHE_DIR)
    data = {"last_saved": datetime.now().isoformat()}
    HISTORY_FILE.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


def is_cache_from_today(history: dict) -> bool:
    """キャッシュが今日のものであれば True を返す。"""
    last_saved = history.get("last_saved")
    if not last_saved:
        return False
    try:
        saved_date = datetime.fromisoformat(last_saved).date()
        return saved_date == datetime.now().date()
    except Exception:
        return False


def load_repo_cache() -> list | None:
    """cache/repositories.json を読み込む。失敗時は None を返す。"""
    if not REPO_CACHE_FILE.exists():
        return None
    try:
        data = json.loads(REPO_CACHE_FILE.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None
    if not isinstance(data, list):
        return None
    for item in data:
        if not isinstance(item, dict) or "name" not in item:
            return None
    return data


def save_repo_cache(repos: list) -> None:
    """リポジトリ一覧を cache/repositories.json に保存する。"""
    _ensure_dir(CACHE_DIR)
    REPO_CACHE_FILE.write_text(
        json.dumps(repos, ensure_ascii=False, indent=2), encoding="utf-8"
    )


_REPO_CONFIG_ENTRY = """\
[[repositories]]
    repository = '{name}'
#    translate_readme = true
#    check_large_files = true
"""


def load_known_repo_names() -> list[str]:
    """config/repositories.toml から既知のリポジトリ名一覧を返す。"""
    if not REPO_CONFIG_FILE.exists():
        return []
    if tomllib is None:
        return []
    try:
        with open(REPO_CONFIG_FILE, "rb") as f:
            cfg = tomllib.load(f)
        return [r["repository"] for r in cfg.get("repositories", []) if "repository" in r]
    except (OSError, ValueError):
        return []


def append_repos_to_config(new_repo_names: list[str]) -> None:
    """新規リポジトリを config/repositories.toml に追記する。"""
    normalized_names = sorted(set(new_repo_names))
    if not normalized_names:
        return
    _ensure_dir(CONFIG_DIR)
    file_is_empty = not REPO_CONFIG_FILE.exists() or REPO_CONFIG_FILE.stat().st_size == 0
    with open(REPO_CONFIG_FILE, "a", encoding="utf-8") as f:
        for i, name in enumerate(normalized_names):
            prefix = "" if file_is_empty and i == 0 else "\n"
            f.write(prefix + _REPO_CONFIG_ENTRY.format(name=name))


def print_repo_config() -> None:
    """config/repositories.toml の設定内容を表示する。"""
    print(f"\n{C.TITLE}{C.BOLD}=== リポジトリ設定 (config/repositories.toml) ==={C.RESET}")
    if not REPO_CONFIG_FILE.exists():
        print(f"  {dim('(未作成: リポジトリ一覧取得後に生成される)')}")
        return
    if tomllib is None:
        print(f"  {dim('(TOML パーサーがないため読み込めない)')}")
        return
    try:
        with open(REPO_CONFIG_FILE, "rb") as f:
            cfg = tomllib.load(f)
    except (OSError, ValueError):
        print(f"  {dim('(読み込み失敗)')}")
        return
    repos_cfg = cfg.get("repositories", [])
    if not repos_cfg:
        print(f"  {dim('(設定なし)')}")
        return
    for r in repos_cfg:
        name = r.get("repository", "?")
        translate  = r.get("translate_readme",  False)
        check_large = r.get("check_large_files", False)
        flags = [
            ok("translate_readme")  if translate   else dim("translate_readme"),
            ok("check_large_files") if check_large else dim("check_large_files"),
        ]
        print(f"  {repo(name)}: {' | '.join(flags)}")
