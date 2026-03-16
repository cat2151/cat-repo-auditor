#!/usr/bin/env python3
"""
check_local_workflows/app.py

README.ja.md を持つ git リポジトリを対象に、
config.toml の sync_filepaths に列挙されたファイルのハッシュを比較する。
欠落しているリポジトリも不一致として報告する。
"""

import sys
from pathlib import Path

try:
    from .checker import check_one
    from .config_loader import load_sync_filepaths
    from .constants import CALL_CHECK_LARGE_FILES_WF, PREREQUISITE
    from .installer import install_large_files_toml
    from .repo_discovery import collect_target_repos
except ImportError:
    from checker import check_one
    from config_loader import load_sync_filepaths
    from constants import CALL_CHECK_LARGE_FILES_WF, PREREQUISITE
    from installer import install_large_files_toml
    from repo_discovery import collect_target_repos


def main() -> None:
    sync_filepaths = load_sync_filepaths()
    parent  = Path.cwd().parent
    siblings = sorted(p for p in parent.iterdir() if p.is_dir())

    print(f"[INFO] 親ディレクトリ   : {parent}")
    print(f"[INFO] 前提条件ファイル : {PREREQUISITE}")
    print(f"[INFO] 対象 sync ファイル: {len(sync_filepaths)} 件")
    print()

    target_repos = collect_target_repos(siblings, PREREQUISITE)
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
