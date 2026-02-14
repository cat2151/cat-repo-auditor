"""Self-update helpers for cat-repo-auditor."""

from __future__ import annotations

import os
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional, Tuple

UPDATE_CHECK_INTERVAL_SECONDS = 60
REPO_ROOT = Path(__file__).resolve().parent.parent

_last_check_time: float = 0.0
_REMOTE_PATTERN = re.compile(r"github\.com[:/](?P<owner>[^/]+)/(?P<repo>[^/]+?)(?:\.git)?$")


def _run_command(args: list[str], cwd: Path | str | None = None) -> subprocess.CompletedProcess[str]:
    """Run a command and return the completed process without raising on error."""
    try:
        return subprocess.run(
            args,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            cwd=cwd,
            check=False,
        )
    except FileNotFoundError as exc:
        return subprocess.CompletedProcess(args=args, returncode=127, stdout="", stderr=str(exc))


def _parse_remote_url(remote_url: str) -> Optional[Tuple[str, str]]:
    """Parse a GitHub remote URL into (owner, repo)."""
    match = _REMOTE_PATTERN.search(remote_url.strip())
    if not match:
        return None
    return match.group("owner"), match.group("repo")


def _get_tracking_branch(repo_root: Path) -> Optional[Tuple[str, str]]:
    """Return (remote, branch) for the current upstream if configured."""
    result = _run_command(
        ["git", "-C", str(repo_root), "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]
    )
    if result.returncode != 0:
        return None

    ref = result.stdout.strip()
    if "/" not in ref:
        return None
    remote, branch = ref.split("/", 1)
    if not remote or not branch:
        return None
    return remote, branch


def _get_remote_repo(repo_root: Path, remote_name: str) -> Optional[Tuple[str, str]]:
    """Return (owner, repo) for the given remote using its URL."""
    result = _run_command(["git", "-C", str(repo_root), "remote", "get-url", remote_name])
    if result.returncode != 0:
        return None
    return _parse_remote_url(result.stdout)


def _get_local_head_sha(repo_root: Path) -> Optional[str]:
    """Return the current HEAD SHA."""
    result = _run_command(["git", "-C", str(repo_root), "rev-parse", "HEAD"])
    if result.returncode != 0:
        return None
    return result.stdout.strip() or None


def _get_remote_latest_sha(owner: str, repo: str, branch: str, cwd: Path) -> Optional[str]:
    """Fetch the latest SHA for the remote branch via gh api."""
    result = _run_command(
        [
            "gh",
            "api",
            f"repos/{owner}/{repo}/commits",
            "-F",
            f"sha={branch}",
            "-F",
            "per_page=1",
            "--jq",
            ".[0].sha",
        ],
        cwd=cwd,
    )
    if result.returncode != 0:
        return None
    sha = result.stdout.strip()
    return sha or None


def _is_worktree_clean(repo_root: Path) -> bool:
    """Check if the worktree has no local modifications."""
    result = _run_command(["git", "-C", str(repo_root), "status", "--porcelain"])
    return result.returncode == 0 and not result.stdout.strip()


def _pull_fast_forward(repo_root: Path, remote_name: str, branch: str) -> bool:
    """Attempt a fast-forward pull; return True on success."""
    result = _run_command(["git", "-C", str(repo_root), "pull", "--ff-only", remote_name, branch])
    if result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip()
        print(f"Auto-update skipped: git pull failed ({message}).")
        return False
    return True


def restart_application() -> None:
    """Restart the current Python process with the same arguments."""
    os.chdir(REPO_ROOT)
    os.execv(sys.executable, [sys.executable] + sys.argv)


def maybe_self_update(repo_root: Path | None = None) -> bool:
    """Check for repository updates and restart the app if new commits are available."""
    global _last_check_time
    if os.getenv("CAT_REPO_AUDITOR_DISABLE_AUTO_UPDATE") == "1":
        return False

    now = time.time()
    if _last_check_time and now - _last_check_time < UPDATE_CHECK_INTERVAL_SECONDS:
        return False
    _last_check_time = now

    repo_root = repo_root or REPO_ROOT

    try:
        tracking = _get_tracking_branch(repo_root)
        if not tracking:
            return False
        remote_name, branch = tracking

        remote_repo = _get_remote_repo(repo_root, remote_name)
        if not remote_repo:
            return False
        owner, repo = remote_repo

        local_sha = _get_local_head_sha(repo_root)
        if not local_sha:
            return False

        remote_sha = _get_remote_latest_sha(owner, repo, branch, repo_root)
        if not remote_sha or remote_sha == local_sha:
            return False

        if not _is_worktree_clean(repo_root):
            print("Auto-update skipped: local changes detected.")
            return False

        if not _pull_fast_forward(repo_root, remote_name, branch):
            return False

        print("Auto-update applied: restarting to use the latest code...")
        restart_application()
        return True
    except Exception:
        return False
