"""cache モジュールのテスト。"""
import sys
import os
import json
from datetime import datetime, timedelta
from pathlib import Path

import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

import cat_repo_auditor.constants as constants_mod
from cat_repo_auditor.cache import (
    _ensure_dir,
    load_history,
    save_history,
    is_cache_from_today,
    load_repo_cache,
    save_repo_cache,
    load_known_repo_names,
    append_repos_to_config,
    print_repo_config,
)


@pytest.fixture(autouse=True)
def patch_paths(tmp_path, monkeypatch):
    """キャッシュ・設定パスを tmp_path に向ける。"""
    cache_dir = tmp_path / "cache"
    config_dir = tmp_path / "config"

    monkeypatch.setattr(constants_mod, "CACHE_DIR", cache_dir)
    monkeypatch.setattr(constants_mod, "HISTORY_FILE", cache_dir / "history.json")
    monkeypatch.setattr(constants_mod, "REPO_CACHE_FILE", cache_dir / "repositories.json")
    monkeypatch.setattr(constants_mod, "CONFIG_DIR", config_dir)
    monkeypatch.setattr(constants_mod, "REPO_CONFIG_FILE", config_dir / "repositories.toml")

    import cat_repo_auditor.cache as cache_mod
    monkeypatch.setattr(cache_mod, "CACHE_DIR", cache_dir)
    monkeypatch.setattr(cache_mod, "HISTORY_FILE", cache_dir / "history.json")
    monkeypatch.setattr(cache_mod, "REPO_CACHE_FILE", cache_dir / "repositories.json")
    monkeypatch.setattr(cache_mod, "CONFIG_DIR", config_dir)
    monkeypatch.setattr(cache_mod, "REPO_CONFIG_FILE", config_dir / "repositories.toml")


def test_ensure_dir_creates_directory(tmp_path):
    target = tmp_path / "new" / "dir"
    _ensure_dir(target)
    assert target.is_dir()


def test_load_history_returns_empty_when_missing():
    result = load_history()
    assert result == {}


def test_save_and_load_history():
    save_history()
    result = load_history()
    assert "last_saved" in result


def test_is_cache_from_today_true():
    assert is_cache_from_today({"last_saved": datetime.now().isoformat()}) is True


def test_is_cache_from_today_false_old_date():
    yesterday = (datetime.now() - timedelta(days=1)).isoformat()
    assert is_cache_from_today({"last_saved": yesterday}) is False


def test_is_cache_from_today_false_missing_key():
    assert is_cache_from_today({}) is False


def test_load_repo_cache_returns_none_when_missing():
    assert load_repo_cache() is None


def test_save_and_load_repo_cache():
    repos = [{"name": "repo1"}, {"name": "repo2"}]
    save_repo_cache(repos)
    result = load_repo_cache()
    assert result == repos


def test_load_repo_cache_returns_none_when_invalid_json(tmp_path, monkeypatch):
    import cat_repo_auditor.cache as cache_mod
    cache_dir = tmp_path / "cache"
    cache_dir.mkdir()
    bad_file = cache_dir / "repositories.json"
    bad_file.write_text("not json", encoding="utf-8")
    monkeypatch.setattr(cache_mod, "REPO_CACHE_FILE", bad_file)
    assert load_repo_cache() is None


def test_load_known_repo_names_returns_empty_when_missing():
    assert load_known_repo_names() == []


def test_append_repos_to_config_creates_file():
    append_repos_to_config(["repo-a", "repo-b"])
    names = load_known_repo_names()
    assert "repo-a" in names
    assert "repo-b" in names


def test_append_repos_to_config_deduplicates():
    append_repos_to_config(["repo-x", "repo-x"])
    names = load_known_repo_names()
    assert names.count("repo-x") == 1


def test_append_repos_to_config_empty_does_nothing():
    append_repos_to_config([])
    assert load_known_repo_names() == []


# ---- print_repo_config ----

def test_print_repo_config_file_missing(capsys):
    # autouse fixture patches REPO_CONFIG_FILE to a path that doesn't exist
    print_repo_config()
    out = capsys.readouterr().out
    assert "未作成" in out


def test_print_repo_config_with_repos(tmp_path, capsys):
    # autouse fixture already patched REPO_CONFIG_FILE to tmp_path/config/repositories.toml
    config_dir = tmp_path / "config"
    config_dir.mkdir()
    (config_dir / "repositories.toml").write_bytes(
        b'[[repositories]]\n    repository = "my-repo"\n'
    )
    print_repo_config()
    out = capsys.readouterr().out
    assert "my-repo" in out


def test_print_repo_config_empty_repos(tmp_path, capsys):
    # Empty TOML file → no [[repositories]] → "(設定なし)"
    config_dir = tmp_path / "config"
    config_dir.mkdir()
    (config_dir / "repositories.toml").write_bytes(b"")
    print_repo_config()
    out = capsys.readouterr().out
    assert "設定なし" in out


def test_print_repo_config_load_failure(tmp_path, capsys):
    # Invalid TOML → "(読み込み失敗)"
    config_dir = tmp_path / "config"
    config_dir.mkdir()
    (config_dir / "repositories.toml").write_bytes(b"[invalid toml !!!")
    print_repo_config()
    out = capsys.readouterr().out
    assert "読み込み失敗" in out
