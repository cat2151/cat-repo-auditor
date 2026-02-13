from __future__ import annotations

import sys
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

from cat_repo_auditor.config import load_config  # noqa: E402


def test_load_config_creates_default_when_missing(tmp_path: Path) -> None:
    config_path = tmp_path / "audit_config.toml"

    config = load_config(config_path)

    assert config["check_items"] == ["README.md", "LICENSE", ".gitignore"]
    assert config["display"]["show_repo_name"] is True
    assert config["display"]["highlight_missing"] is True
    assert config_path.exists()
    assert "# Repository Auditor Configuration" in config_path.read_text(encoding="utf-8")


def test_load_config_reads_existing_config(tmp_path: Path) -> None:
    config_path = tmp_path / "audit_config.toml"
    config_content = """check_items = ["README.md", "CONTRIBUTING.md"]

[display]
show_repo_name = false
highlight_missing = false
"""
    config_path.write_text(config_content, encoding="utf-8")

    config = load_config(config_path)

    assert config["check_items"] == ["README.md", "CONTRIBUTING.md"]
    assert config["display"]["show_repo_name"] is False
    assert config["display"]["highlight_missing"] is False
