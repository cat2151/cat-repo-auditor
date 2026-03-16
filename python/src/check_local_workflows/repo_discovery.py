import subprocess
from pathlib import Path


def is_git_repo(path: Path) -> bool:
    result = subprocess.run(
        ["git", "-C", str(path), "rev-parse", "--is-inside-work-tree"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0


def is_target_repo(path: Path, prerequisite: str) -> bool:
    """git リポジトリかつ prerequisite ファイルを持つかどうかを返す。"""
    return is_git_repo(path) and (path / prerequisite).exists()


def collect_target_repos(siblings: list[Path], prerequisite: str) -> list[Path]:
    """処理対象リポジトリの一覧を返す。"""
    return [d for d in siblings if is_target_repo(d, prerequisite)]
