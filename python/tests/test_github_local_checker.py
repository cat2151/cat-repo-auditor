"""github_local_checker モジュールのテスト。"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from github_local_checker.constants import (
    STATUS_PULLABLE,
    STATUS_DIVERGED,
    STATUS_UP_TO_DATE,
    STATUS_UNKNOWN,
    STATUS_LABEL,
    DEFAULT_CONFIG,
    DEFAULT_OUTPUT,
    _COLOR,
    _RESET,
)
from github_local_checker.checker import classify
from github_local_checker.git_utils import is_target_repo
from github_local_checker.display import colored


# ---------------------------------------------------------------------------
# classify のテスト
# ---------------------------------------------------------------------------

def test_classify_pullable():
    assert classify(False, 3, 0) == STATUS_PULLABLE


def test_classify_diverged():
    assert classify(False, 2, 1) == STATUS_DIVERGED


def test_classify_diverged_dirty():
    assert classify(True, 2, 1) == STATUS_DIVERGED


def test_classify_up_to_date():
    assert classify(False, 0, 0) == STATUS_UP_TO_DATE


def test_classify_up_to_date_with_ahead():
    assert classify(False, 0, 3) == STATUS_UP_TO_DATE


def test_classify_unknown_negative():
    assert classify(False, -1, -1) == STATUS_UNKNOWN


def test_classify_unknown_dirty_behind():
    assert classify(True, 3, 0) == STATUS_UNKNOWN


# ---------------------------------------------------------------------------
# is_target_repo のテスト
# ---------------------------------------------------------------------------

def test_is_target_repo_https():
    assert is_target_repo("https://github.com/myuser/myrepo.git", "myuser") is True


def test_is_target_repo_ssh():
    assert is_target_repo("git@github.com:myuser/myrepo.git", "myuser") is True


def test_is_target_repo_other_user():
    assert is_target_repo("https://github.com/otheruser/myrepo.git", "myuser") is False


def test_is_target_repo_not_github():
    assert is_target_repo("https://gitlab.com/myuser/myrepo.git", "myuser") is False


def test_is_target_repo_case_insensitive():
    assert is_target_repo("https://github.com/MyUser/myrepo.git", "myuser") is True


# ---------------------------------------------------------------------------
# colored のテスト
# ---------------------------------------------------------------------------

def test_colored_pullable():
    result = colored("text", STATUS_PULLABLE)
    assert _COLOR[STATUS_PULLABLE] in result
    assert _RESET in result
    assert "text" in result


def test_colored_unknown_status():
    result = colored("text", "nonexistent_status")
    assert result == f"text{_RESET}"


# ---------------------------------------------------------------------------
# 定数のテスト
# ---------------------------------------------------------------------------

def test_status_label_keys():
    for status in [STATUS_PULLABLE, STATUS_DIVERGED, STATUS_UP_TO_DATE, STATUS_UNKNOWN]:
        assert status in STATUS_LABEL


def test_default_config():
    assert DEFAULT_CONFIG == "config.toml"


def test_default_output():
    assert DEFAULT_OUTPUT == "github_local_checker_result.json"
