from collections import Counter
from pathlib import Path

try:
    from .file_utils import bytes_sha256, file_sha256
    from .git_utils import git_show_remote_file
except ImportError:
    from file_utils import bytes_sha256, file_sha256
    from git_utils import git_show_remote_file


def compute_majority_hash(
    target_repos: list[Path],
    sync_filepath: Path,
) -> str | None:
    """sync_filepath を持つリポジトリのハッシュから多数派を返す。"""
    hashes = [
        file_sha256(d / sync_filepath)
        for d in target_repos
        if (d / sync_filepath).exists()
    ]
    if not hashes:
        return None
    return Counter(hashes).most_common(1)[0][0]


def find_master_repo(
    target_repos: list[Path],
    sync_filepath: Path,
    majority_hash: str | None,
    master_repo_name: str | None = None,
) -> Path | None:
    """コピー元リポジトリを返す。"""
    if master_repo_name:
        for d in target_repos:
            if d.name == master_repo_name:
                if (d / sync_filepath).exists():
                    return d
                print(
                    f"[WARN] master_repo '{master_repo_name}' は "
                    f"'{sync_filepath}' を含まないため、多数派ハッシュのリポジトリにフォールバックします。"
                )
                break

    if majority_hash is None:
        return None

    return next(
        (
            d
            for d in target_repos
            if (d / sync_filepath).exists()
            and file_sha256(d / sync_filepath) == majority_hash
        ),
        None,
    )


def get_uncommitted_vs_remote(
    repo_dir: Path,
    sync_filepaths: list[Path],
) -> list[tuple[Path, str, str, bytes]]:
    """worktree のファイルと remote HEAD をバイト比較し差異一覧を返す。"""
    results = []
    for fp in sync_filepaths:
        local_path = repo_dir / fp
        if not local_path.exists():
            continue

        remote_bytes = git_show_remote_file(repo_dir, fp)
        if remote_bytes is None:
            continue

        local_hash = file_sha256(local_path)
        remote_hash = bytes_sha256(remote_bytes)
        if local_hash != remote_hash:
            results.append((fp, local_hash, remote_hash, remote_bytes))

    return results


def detect_outliers(
    target_repos: list[Path],
    sync_filepath: Path,
) -> tuple[str | None, list[Path]]:
    """多数派ハッシュを算出し、欠落または不一致リポジトリ一覧を返す。"""
    majority_hash = compute_majority_hash(target_repos, sync_filepath)
    if majority_hash is None:
        print(f"  [SKIP] {sync_filepath.as_posix()} : 全リポジトリで欠落")
        return None, []

    outliers = []
    for d in target_repos:
        fp = d / sync_filepath
        if not fp.exists():
            outliers.append(d)
        elif file_sha256(fp) != majority_hash:
            outliers.append(d)

    if not outliers:
        print(f"  [OK]   {sync_filepath.as_posix()} : 全一致")
    else:
        print(f"  [DIFF] {sync_filepath.as_posix()} : 多数派 {majority_hash}")
        for d in outliers:
            fp = d / sync_filepath
            status = "(欠落)" if not fp.exists() else file_sha256(fp)
            print(f"         対象: {d.name}  {status}")

    return majority_hash, outliers
