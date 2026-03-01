import subprocess
from pathlib import Path


def run_git(args: list[str], cwd: str) -> tuple[int, str, str]:
    """git コマンドを実行し (returncode, stdout, stderr) を返す。"""
    result = subprocess.run(
        ["git"] + args,
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    return result.returncode, result.stdout.strip(), result.stderr.strip()


def is_git_repo(path: str) -> bool:
    rc, _, _ = run_git(["rev-parse", "--git-dir"], path)
    return rc == 0


def get_remote_url(path: str) -> str | None:
    rc, out, _ = run_git(["remote", "get-url", "origin"], path)
    return out if rc == 0 and out else None


def is_target_repo(remote_url: str, github_username: str) -> bool:
    """
    remote URL が指定ユーザーの GitHub リポジトリかを判定する。
    HTTPS: https://github.com/<user>/...
    SSH  : git@github.com:<user>/...
    """
    lower    = remote_url.lower()
    user_low = github_username.lower()
    if "github.com" not in lower:
        return False
    return (
        f"github.com/{user_low}/" in lower
        or f"github.com:{user_low}/" in lower
    )


def is_dirty(path: str) -> bool:
    """未コミットの変更があれば True。git が動かなければ True（dirty 扱い）。"""
    rc, out, _ = run_git(["status", "--porcelain"], path)
    return bool(out) if rc == 0 else True


def get_current_branch(path: str) -> str | None:
    rc, out, _ = run_git(["rev-parse", "--abbrev-ref", "HEAD"], path)
    return out if rc == 0 else None


def fetch_remote(path: str) -> tuple[bool, str | None]:
    """
    origin を fetch する。
    戻り値: (成功フラグ, エラーメッセージ or None)
    """
    rc, _, err = run_git(["fetch", "origin", "--quiet"], path)
    if rc != 0:
        msg = f"git fetch 失敗: {err}" if err else "git fetch 失敗"
        return False, msg
    return True, None


def pull_repo(path: str) -> tuple[bool, str]:
    """
    git pull を実行する（fast-forward のみ）。
    戻り値: (成功フラグ, stdout または エラーメッセージ)
    pullable 判定済み（dirty=False, ahead=0）のリポジトリにのみ呼ぶこと。
    """
    rc, out, err = run_git(["pull", "--ff-only"], path)
    if rc != 0:
        return False, err or "git pull 失敗"
    return True, out or "Already up to date."


def get_behind_ahead(path: str, branch: str) -> tuple[int, int]:
    """
    origin/<branch> に対して (behind, ahead) を返す。
    取得不能なら (-1, -1)。
    """
    tracking = f"origin/{branch}"
    rc, out, _ = run_git(
        ["rev-list", "--left-right", "--count", f"{tracking}...HEAD"],
        path,
    )
    if rc != 0:
        return -1, -1
    parts = out.split()
    if len(parts) != 2:
        return -1, -1
    return int(parts[0]), int(parts[1])
