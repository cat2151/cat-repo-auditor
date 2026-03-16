"""
パス定数とアプリケーションレベルの定数を一元管理する。
"""

from pathlib import Path

CACHE_DIR       = Path("cache")
HISTORY_FILE    = CACHE_DIR / "history.json"
REPO_CACHE_FILE = CACHE_DIR / "repositories.json"
CONFIG_DIR      = Path("config")
REPO_CONFIG_FILE = CONFIG_DIR / "repositories.toml"

DEEPWIKI_PATTERNS = ["deepwiki.com", "deepwiki", "DeepWiki"]
