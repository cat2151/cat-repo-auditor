import shutil
from pathlib import Path

try:
    from .constants import CHECK_LARGE_FILES_CONFIG
except ImportError:
    from constants import CHECK_LARGE_FILES_CONFIG


def find_latest_large_files_toml(target_repos: list[Path]) -> Path | None:
    """
    target_repos の中から最も新しい CHECK_LARGE_FILES_CONFIG を返す。
    1件も存在しなければ None を返す。
    """
    candidates = [
        d / CHECK_LARGE_FILES_CONFIG
        for d in target_repos
        if (d / CHECK_LARGE_FILES_CONFIG).exists()
    ]
    if not candidates:
        return None
    return max(candidates, key=lambda p: p.stat().st_mtime)


def install_large_files_toml(target_repos: list[Path]) -> bool:
    """
    CHECK_LARGE_FILES_CONFIG が欠落しているリポジトリへ、
    最新ファイルをコピーしてインストールする。
    インストール元が見つからない場合のみ False を返す。
    """
    print(f"=== {CHECK_LARGE_FILES_CONFIG.as_posix()} (install) ===")

    latest = find_latest_large_files_toml(target_repos)
    if latest is None:
        print("[WARN] すべてのリポジトリで check-large-files.toml が欠落している。インストール元が見つからない。")
        print()
        return False

    print(f"  インストール元 (最新): {latest.parent.parent.name}/{latest.relative_to(latest.parent.parent).as_posix()}")
    print()

    for d in target_repos:
        dest = d / CHECK_LARGE_FILES_CONFIG
        if dest.exists():
            print(f"  [OK]   {d.name}")
        else:
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(latest, dest)
            print(f"  [COPY] {d.name}  <- インストール済み")

    print()
    return True
