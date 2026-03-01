#!/usr/bin/env python3
"""
check_local_workflows.py

README.ja.md を持つ git リポジトリを対象に、
config.toml の sync_filepaths に列挙されたファイルのハッシュを比較する。
欠落しているリポジトリも不一致として報告する。
"""

import sys
import hashlib
import shutil
import subprocess
import tomllib
from collections import Counter
from pathlib import Path


CONFIG_FILE  = Path(__file__).parent / "config.toml"
PREREQUISITE = "README.ja.md"

CALL_CHECK_LARGE_FILES_WF = Path(".github/workflows/call-check-large-files.yml")
CHECK_LARGE_FILES_CONFIG  = Path(".github/check-large-files.toml")


# ---------------------------------------------------------------------------
# 設定読み込み
# ---------------------------------------------------------------------------

def load_sync_filepaths() -> list[Path]:
    """config.toml から sync_filepaths を読み込む。"""
    if not CONFIG_FILE.exists():
        print(f"[ERROR] config.toml が見つからない: {CONFIG_FILE}")
        sys.exit(1)
    with open(CONFIG_FILE, "rb") as f:
        config = tomllib.load(f)
    paths = config.get("sync", {}).get("sync_filepaths", [])
    if not paths:
        print("[ERROR] config.toml に sync_filepaths が設定されていない。")
        sys.exit(1)
    return [Path(p) for p in paths]


# ---------------------------------------------------------------------------
# 対象リポジトリ判定
# ---------------------------------------------------------------------------

def is_git_repo(path: Path) -> bool:
    result = subprocess.run(
        ["git", "-C", str(path), "rev-parse", "--is-inside-work-tree"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0


def is_target_repo(path: Path) -> bool:
    """git リポジトリかつ PREREQUISITE ファイルを持つかどうかを返す。"""
    return is_git_repo(path) and (path / PREREQUISITE).exists()


def collect_target_repos(siblings: list[Path]) -> list[Path]:
    """処理対象リポジトリの一覧を返す。"""
    return [d for d in siblings if is_target_repo(d)]


# ---------------------------------------------------------------------------
# ファイルユーティリティ
# ---------------------------------------------------------------------------

def file_sha256(path: Path) -> str:
    """ファイルの SHA-256 ハッシュを返す（バイト列ベース）。"""
    return hashlib.sha256(path.read_bytes()).hexdigest()


# ---------------------------------------------------------------------------
# check-large-files.toml インストール
# ---------------------------------------------------------------------------

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


# ---------------------------------------------------------------------------
# ハッシュ比較
# ---------------------------------------------------------------------------

def check_one(sync_filepath: Path, target_repos: list[Path]) -> bool:
    """
    1ファイルについてハッシュ比較を行い、全一致なら True を返す。
    欠落リポジトリも不一致として扱う。
    """
    print(f"=== {sync_filepath.as_posix()} ===")

    hashes: dict[Path, str] = {}
    for d in target_repos:
        fp = d / sync_filepath
        hashes[d] = file_sha256(fp) if fp.exists() else "(欠落)"

    print("[HASH]")
    for d, digest in hashes.items():
        print(f"  {d.name:<30}  {digest}")

    existing = [h for h in hashes.values() if h != "(欠落)"]
    if not existing:
        print("[WARN] すべてのリポジトリでファイルが欠落している。")
        print()
        return False

    majority_hash = Counter(existing).most_common(1)[0][0]
    all_ok = all(h == majority_hash for h in hashes.values())

    if all_ok:
        print("[OK] すべて一致。")
        print()
        return True
    else:
        print("[WARN] 不一致あり。")
        print(f"  基準ハッシュ (最多一致): {majority_hash}")
        for d, digest in sorted(hashes.items()):
            if digest != majority_hash:
                print(f"  対象: {d.name:<30}  {digest}")
        print()
        return False


# ---------------------------------------------------------------------------
# エントリーポイント
# ---------------------------------------------------------------------------

def main():
    sync_filepaths = load_sync_filepaths()
    parent  = Path.cwd().parent
    siblings = sorted(p for p in parent.iterdir() if p.is_dir())

    print(f"[INFO] 親ディレクトリ   : {parent}")
    print(f"[INFO] 前提条件ファイル : {PREREQUISITE}")
    print(f"[INFO] 対象 sync ファイル: {len(sync_filepaths)} 件")
    print()

    target_repos = collect_target_repos(siblings)
    if not target_repos:
        print(f"[WARN] {PREREQUISITE} を持つリポジトリが見つからなかった。")
        sys.exit(1)

    print(f"[INFO] 処理対象リポジトリ: {len(target_repos)} 件")
    for d in target_repos:
        print(f"  {d.name}")
    print()

    all_ok = all(check_one(fp, target_repos) for fp in sync_filepaths)

    if CALL_CHECK_LARGE_FILES_WF in sync_filepaths:
        toml_ok = install_large_files_toml(target_repos)
        all_ok = all_ok and toml_ok

    if all_ok:
        print("すべての対象ファイルで一致を確認した。")
    else:
        print("不一致が検出された。sync_workflows.py を実行して同期すること。")
        sys.exit(2)


if __name__ == "__main__":
    main()
