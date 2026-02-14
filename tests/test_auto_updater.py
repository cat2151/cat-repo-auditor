from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(PROJECT_ROOT / "src"))

import cat_repo_auditor.auto_updater as auto_updater  # noqa: E402
from cat_repo_auditor.auto_updater import maybe_self_update  # noqa: E402


def test_run_command_handles_missing_binary():
    result = auto_updater._run_command(["__definitely_missing_command__"])  # type: ignore[attr-defined]

    assert result.returncode != 0


def test_maybe_self_update_skips_without_tracking(monkeypatch):
    auto_updater._last_check_time = 0  # type: ignore[attr-defined]
    monkeypatch.setattr(auto_updater, "_get_tracking_branch", lambda repo_root: None)
    monkeypatch.setattr(auto_updater, "_get_remote_repo", lambda *args, **kwargs: ("owner", "repo"))
    restart_called = {"value": False}
    monkeypatch.setattr(auto_updater, "restart_application", lambda: restart_called.update(value=True))

    updated = maybe_self_update(repo_root=Path("."))

    assert updated is False
    assert restart_called["value"] is False


def test_maybe_self_update_updates_when_remote_newer(monkeypatch):
    auto_updater._last_check_time = 0  # type: ignore[attr-defined]
    monkeypatch.setattr(auto_updater, "_get_tracking_branch", lambda repo_root: ("origin", "main"))
    monkeypatch.setattr(auto_updater, "_get_remote_repo", lambda *args, **kwargs: ("owner", "repo"))
    monkeypatch.setattr(auto_updater, "_get_local_head_sha", lambda repo_root: "oldsha")
    monkeypatch.setattr(auto_updater, "_get_remote_latest_sha", lambda *args, **kwargs: "newsha")
    monkeypatch.setattr(auto_updater, "_is_worktree_clean", lambda repo_root: True)
    monkeypatch.setattr(auto_updater, "_pull_fast_forward", lambda *args, **kwargs: True)
    restart_called = {"value": False}
    monkeypatch.setattr(auto_updater, "restart_application", lambda: restart_called.update(value=True))

    updated = maybe_self_update(repo_root=Path("."))

    assert updated is True
    assert restart_called["value"] is True
