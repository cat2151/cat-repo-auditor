from pathlib import Path

try:
    from .git_utils import is_git_repo
except ImportError:
    from git_utils import is_git_repo


def is_target_repo(path: Path, prerequisite: str) -> bool:
    """git リポジトリかつ prerequisite ファイルを持つかどうかを返す。"""
    return is_git_repo(path) and (path / prerequisite).exists()


def collect_target_repos(siblings: list[Path], prerequisite: str) -> list[Path]:
    """処理対象リポジトリの一覧を返す。"""
    return [d for d in siblings if is_target_repo(d, prerequisite)]
