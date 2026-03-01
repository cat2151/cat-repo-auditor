"""config_loader モジュールのテスト。"""
import sys
import os

import pytest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from cat_repo_auditor.config_loader import load_config


def test_load_config_missing_file(tmp_path):
    with pytest.raises(SystemExit):
        load_config(str(tmp_path / "nonexistent.toml"))


def test_load_config_missing_github_user(tmp_path):
    cfg_file = tmp_path / "config.toml"
    cfg_file.write_bytes(b'other_key = "value"\n')
    with pytest.raises(SystemExit):
        load_config(str(cfg_file))


def test_load_config_valid(tmp_path):
    cfg_file = tmp_path / "config.toml"
    cfg_file.write_bytes(b'github_user = "testuser"\n')
    result = load_config(str(cfg_file))
    assert result["github_user"] == "testuser"
