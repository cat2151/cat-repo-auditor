"""constants モジュールのテスト。"""
import sys
import os
from pathlib import Path

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from cat_repo_auditor.constants import (
    CACHE_DIR,
    HISTORY_FILE,
    REPO_CACHE_FILE,
    CONFIG_DIR,
    REPO_CONFIG_FILE,
    DEEPWIKI_PATTERNS,
)


def test_cache_dir():
    assert CACHE_DIR == Path("cache")


def test_history_file():
    assert HISTORY_FILE == Path("cache") / "history.json"


def test_repo_cache_file():
    assert REPO_CACHE_FILE == Path("cache") / "repositories.json"


def test_config_dir():
    assert CONFIG_DIR == Path("config")


def test_repo_config_file():
    assert REPO_CONFIG_FILE == Path("config") / "repositories.toml"


def test_deepwiki_patterns_contains_expected():
    assert "deepwiki.com" in DEEPWIKI_PATTERNS
    assert "DeepWiki" in DEEPWIKI_PATTERNS
