"""check_local_workflows モジュールのテスト。"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from check_local_workflows.constants import (
    PREREQUISITE,
    CALL_CHECK_LARGE_FILES_WF,
    CHECK_LARGE_FILES_CONFIG,
    CONFIG_FILE,
)
from check_local_workflows.file_utils import file_sha256
from check_local_workflows.checker import check_one
from check_local_workflows.installer import find_latest_large_files_toml


# ---------------------------------------------------------------------------
# 定数のテスト
# ---------------------------------------------------------------------------

def test_prerequisite():
    assert PREREQUISITE == "README.ja.md"


def test_call_check_large_files_wf():
    from pathlib import Path
    assert CALL_CHECK_LARGE_FILES_WF == Path(".github/workflows/call-check-large-files.yml")


def test_check_large_files_config():
    from pathlib import Path
    assert CHECK_LARGE_FILES_CONFIG == Path(".github/check-large-files.toml")


def test_config_file_is_path():
    from pathlib import Path
    assert isinstance(CONFIG_FILE, Path)
    assert CONFIG_FILE.name == "config.toml"


# ---------------------------------------------------------------------------
# file_sha256 のテスト
# ---------------------------------------------------------------------------

def test_file_sha256(tmp_path):
    import hashlib
    content = b"hello world"
    f = tmp_path / "test.txt"
    f.write_bytes(content)
    expected = hashlib.sha256(content).hexdigest()
    assert file_sha256(f) == expected


def test_file_sha256_empty(tmp_path):
    import hashlib
    f = tmp_path / "empty.txt"
    f.write_bytes(b"")
    expected = hashlib.sha256(b"").hexdigest()
    assert file_sha256(f) == expected


# ---------------------------------------------------------------------------
# find_latest_large_files_toml のテスト
# ---------------------------------------------------------------------------

def test_find_latest_large_files_toml_none(tmp_path):
    repos = [tmp_path / "repo1", tmp_path / "repo2"]
    for r in repos:
        r.mkdir()
    result = find_latest_large_files_toml(repos)
    assert result is None


def test_find_latest_large_files_toml_found(tmp_path):
    import time
    repo1 = tmp_path / "repo1"
    repo2 = tmp_path / "repo2"
    repo1.mkdir()
    repo2.mkdir()

    toml1 = repo1 / CHECK_LARGE_FILES_CONFIG
    toml1.parent.mkdir(parents=True, exist_ok=True)
    toml1.write_text("first")

    time.sleep(0.01)

    toml2 = repo2 / CHECK_LARGE_FILES_CONFIG
    toml2.parent.mkdir(parents=True, exist_ok=True)
    toml2.write_text("second")

    result = find_latest_large_files_toml([repo1, repo2])
    assert result == toml2


# ---------------------------------------------------------------------------
# check_one のテスト
# ---------------------------------------------------------------------------

def test_check_one_all_match(tmp_path, capsys):
    from pathlib import Path

    repo1 = tmp_path / "repo1"
    repo2 = tmp_path / "repo2"
    repo1.mkdir()
    repo2.mkdir()

    content = b"same content"
    fp = Path(".github/workflows/some.yml")
    for repo in [repo1, repo2]:
        f = repo / fp
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_bytes(content)

    result = check_one(fp, [repo1, repo2])
    assert result is True
    out = capsys.readouterr().out
    assert "[OK]" in out


def test_check_one_mismatch(tmp_path, capsys):
    from pathlib import Path

    repo1 = tmp_path / "repo1"
    repo2 = tmp_path / "repo2"
    repo1.mkdir()
    repo2.mkdir()

    fp = Path(".github/workflows/some.yml")
    (repo1 / fp).parent.mkdir(parents=True, exist_ok=True)
    (repo1 / fp).write_bytes(b"content A")
    (repo2 / fp).parent.mkdir(parents=True, exist_ok=True)
    (repo2 / fp).write_bytes(b"content B")

    result = check_one(fp, [repo1, repo2])
    assert result is False
    out = capsys.readouterr().out
    assert "[WARN]" in out


def test_check_one_missing_file(tmp_path, capsys):
    from pathlib import Path

    repo1 = tmp_path / "repo1"
    repo2 = tmp_path / "repo2"
    repo1.mkdir()
    repo2.mkdir()

    fp = Path(".github/workflows/some.yml")
    # only repo1 has the file
    (repo1 / fp).parent.mkdir(parents=True, exist_ok=True)
    (repo1 / fp).write_bytes(b"content")

    result = check_one(fp, [repo1, repo2])
    assert result is False
    out = capsys.readouterr().out
    assert "[WARN]" in out


def test_check_one_all_missing(tmp_path, capsys):
    from pathlib import Path

    repo1 = tmp_path / "repo1"
    repo2 = tmp_path / "repo2"
    repo1.mkdir()
    repo2.mkdir()

    fp = Path(".github/workflows/nonexistent.yml")
    result = check_one(fp, [repo1, repo2])
    assert result is False
    out = capsys.readouterr().out
    assert "欠落" in out
