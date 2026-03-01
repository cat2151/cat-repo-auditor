import subprocess
import sys
from pathlib import Path


def git_run(
    args: list[str],
    cwd: Path,
    timeout: int | None = None,
) -> subprocess.CompletedProcess:
    """git コマンドを実行し、失敗時はエラーを表示して終了する。"""
    cmd = ["git"] + args
    try:
        result = subprocess.run(
            cmd,
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        print(f"[ERROR] コマンドがタイムアウトした: {' '.join(cmd)} (timeout={timeout}s)")
        sys.exit(1)

    if result.returncode != 0:
        print(f"[ERROR] コマンド失敗: {' '.join(cmd)}")
        if result.stderr.strip():
            print(result.stderr.strip())
        if result.stdout.strip():
            print(result.stdout.strip())
        sys.exit(result.returncode)

    return result


def is_git_repo(path: Path) -> bool:
    """指定パスが git リポジトリかどうかを返す。"""
    result = subprocess.run(
        ["git", "-C", str(path), "rev-parse", "--is-inside-work-tree"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def git_fetch(repo_dir: Path) -> None:
    """origin を fetch する。"""
    git_run(["-C", str(repo_dir), "fetch", "origin"], cwd=repo_dir, timeout=30)


def git_get_upstream_ref(repo_dir: Path) -> str | None:
    """現在ブランチの upstream ref（例: origin/main）を返す。未設定なら None。"""
    result = subprocess.run(
        ["git", "-C", str(repo_dir), "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    if result.returncode != 0:
        return None

    ref = result.stdout.strip()
    return ref or None


def git_show_remote_file(repo_dir: Path, filepath: Path) -> bytes | None:
    """remote のファイル内容をバイト列で返す。ファイル未存在時は None。"""
    upstream = git_get_upstream_ref(repo_dir)
    candidates: list[str] = []
    if upstream:
        candidates.append(upstream)
    candidates.extend(["origin/main", "origin/master"])

    seen: set[str] = set()
    for remote_ref in candidates:
        if remote_ref in seen:
            continue
        seen.add(remote_ref)

        spec = f"{remote_ref}:{filepath.as_posix()}"
        try:
            result = subprocess.run(
                ["git", "--no-pager", "-C", str(repo_dir), "show", spec],
                capture_output=True,
                timeout=10,
                check=False,
            )
        except subprocess.TimeoutExpired:
            print(f"[ERROR] git show がタイムアウトした: {repo_dir.name} {spec}")
            sys.exit(1)

        if result.returncode == 0:
            return result.stdout

        stderr_text = result.stderr.decode("utf-8", errors="replace")
        if (
            "does not exist in" in stderr_text
            or "Path '" in stderr_text
            or "exists on disk, but not in" in stderr_text
        ):
            continue

        print(f"[ERROR] git show 失敗: {repo_dir.name} {spec}")
        if stderr_text.strip():
            print(stderr_text.strip())
        sys.exit(result.returncode if result.returncode != 0 else 1)

    return None


def git_add(filepath: Path, repo_dir: Path) -> None:
    """ファイルを git add する。"""
    git_run(["-C", str(repo_dir), "add", filepath.as_posix()], cwd=repo_dir)


def git_has_staged_changes(repo_dir: Path) -> bool:
    """ステージに差分があれば True を返す。"""
    result = subprocess.run(
        ["git", "-C", str(repo_dir), "diff", "--cached", "--quiet"],
        capture_output=True,
        check=False,
    )
    return result.returncode == 1


def git_commit(message: str, repo_dir: Path) -> None:
    """git commit する。"""
    git_run(["-C", str(repo_dir), "commit", "-m", message], cwd=repo_dir)


def git_push(repo_dir: Path) -> None:
    """git push する。"""
    git_run(["-C", str(repo_dir), "push"], cwd=repo_dir)
