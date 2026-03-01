"""colors モジュールのテスト。"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from cat_repo_auditor.colors import C, ok, ng, head, dim, repo, hl


def test_c_has_reset():
    assert C.RESET == "\033[0m"


def test_ok_wraps_with_ok_grn():
    result = ok("text")
    assert C.OK_GRN in result
    assert "text" in result
    assert C.RESET in result


def test_ng_wraps_with_ng_red():
    result = ng("text")
    assert C.NG_RED in result
    assert "text" in result
    assert C.RESET in result


def test_head_wraps_with_title_and_bold():
    result = head("text")
    assert C.TITLE in result
    assert C.BOLD in result
    assert "text" in result
    assert C.RESET in result


def test_dim_wraps_with_dim():
    result = dim("text")
    assert C.DIM in result
    assert "text" in result
    assert C.RESET in result


def test_repo_wraps_with_repo():
    result = repo("text")
    assert C.REPO in result
    assert "text" in result
    assert C.RESET in result


def test_hl_wraps_with_orange_and_bold():
    result = hl("text")
    assert C.ORANGE in result
    assert C.BOLD in result
    assert "text" in result
    assert C.RESET in result
