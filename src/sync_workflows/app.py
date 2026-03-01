#!/usr/bin/env python3
"""
sync_workflows/app.py

README.ja.md を持つ git リポジトリを対象に、
config.toml の sync_filepaths に列挙されたファイルについて
  - 欠落している
  - コピー元（master_repo または多数派）と異なる
のいずれかであれば、コピー元ファイルをコピーして commit & push する。

差分は difft（利用可能な場合）または diff で可視化する。
実施前に y[/n] で確認プロンプトが表示される。
"""

import sys
import tempfile
from collections import defaultdict
from pathlib import Path

try:
    from .config_loader import load_sync_config
    from .constants import COMMIT_MSG, PREREQUISITE
    from .diff_utils import show_difft
    from .file_utils import copy_file
    from .git_utils import (
        git_add,
        git_commit,
        git_fetch,
        git_has_staged_changes,
        git_push,
    )
    from .planner import (
        detect_outliers,
        find_master_repo,
        get_uncommitted_vs_remote,
    )
    from .repo_discovery import collect_target_repos
except ImportError:
    from config_loader import load_sync_config
    from constants import COMMIT_MSG, PREREQUISITE
    from diff_utils import show_difft
    from file_utils import copy_file
    from git_utils import (
        git_add,
        git_commit,
        git_fetch,
        git_has_staged_changes,
        git_push,
    )
    from planner import detect_outliers, find_master_repo, get_uncommitted_vs_remote
    from repo_discovery import collect_target_repos


def show_remote_local_status(
    target_repos: list[Path],
    sync_filepaths: list[Path],
) -> dict[Path, list[tuple[Path, str, str, bytes]]]:
    """対象リポジトリを fetch し、remote/local の差分状態を表示する。"""
    print("=" * 60)
    print("[PHASE 1] remote vs local 差分チェック")
    print("=" * 60)
    print(f"  fetch 対象: {len(target_repos)} 件")
    for d in target_repos:
        print(f"    {d.name}")
    print()

    uncommitted_map: dict[Path, list[tuple[Path, str, str, bytes]]] = {}

    for repo_dir in target_repos:
        print(f"  {repo_dir.name} ... fetch中", end=" ", flush=True)
        git_fetch(repo_dir)
        print("完了")

        diffs = get_uncommitted_vs_remote(repo_dir, sync_filepaths)
        if not diffs:
            print("    [OK] 差分なし")
            continue

        for fp, lh, rh, remote_bytes in diffs:
            print(f"    [!] 差分あり: {fp.as_posix()}")
            print(f"        local  : {lh}")
            print(f"        remote : {rh}")
            print("        ※ localの変更をremoteにcommit & pushする予定")
            with tempfile.NamedTemporaryFile(
                suffix=fp.suffix or ".txt", delete=False, prefix="remote_"
            ) as tmp:
                tmp.write(remote_bytes)
                tmp_path = Path(tmp.name)
            try:
                print("        --- 差分内容 (remote → local) ---")
                show_difft(tmp_path, repo_dir / fp, label_a="remote", label_b="local")
            finally:
                tmp_path.unlink(missing_ok=True)
        uncommitted_map[repo_dir] = diffs

    print()
    if uncommitted_map:
        print("[WARN] 未commit の差分が検出された。")
    else:
        print("[OK] すべてのリポジトリで remote/local は一致している。")
    print()

    return uncommitted_map


def confirm_action(repo_names: list[str], action: str) -> bool:
    """対象リポジトリ名と操作内容を表示し、確認を求める。y なら True を返す。"""
    print(f"対象リポジトリ: {len(repo_names)} 件")
    for name in repo_names:
        print(f"  {name}")
    print()
    answer = input(f"{action} [y/N]: ")
    return answer.strip().lower() == "y"


def commit_and_push_repo(
    repo_dir: Path,
    sync_filepaths: list[Path],
) -> None:
    """コピー不要・既存ファイルを add → commit → push する。"""
    print(f"--- {repo_dir.name} ---")

    for fp in sync_filepaths:
        git_add(fp, repo_dir)
        print(f"  [ADD]  {fp.as_posix()}")

    if not git_has_staged_changes(repo_dir):
        print("  [SKIP] ステージに差分なし。commit をスキップする。")
        return

    git_commit(COMMIT_MSG, repo_dir)
    print(f"  [COMMIT] '{COMMIT_MSG}'")

    git_push(repo_dir)
    print("  [PUSH] 完了")
    print()


def sync_repo(
    repo_dir: Path,
    file_pairs: list[tuple[Path, Path]],
) -> None:
    """多数派からコピーして add・commit・push をまとめて行う。"""
    print(f"--- {repo_dir.name} ---")

    for sync_filepath, master_fp in file_pairs:
        dest = repo_dir / sync_filepath
        copy_file(master_fp, dest)
        print(f"  [COPY] {sync_filepath.as_posix()}")
        git_add(sync_filepath, repo_dir)
        print(f"  [ADD]  {sync_filepath.as_posix()}")

    if not git_has_staged_changes(repo_dir):
        print("  [SKIP] ステージに差分なし。commit をスキップする。")
        return

    git_commit(COMMIT_MSG, repo_dir)
    print(f"  [COMMIT] '{COMMIT_MSG}'")

    git_push(repo_dir)
    print("  [PUSH] 完了")
    print()


def main() -> None:
    sync_filepaths, master_repo_name = load_sync_config()
    parent = Path.cwd().parent
    siblings = sorted(p for p in parent.iterdir() if p.is_dir())

    print(f"[INFO] 親ディレクトリ    : {parent}")
    print(f"[INFO] 前提条件ファイル  : {PREREQUISITE}")
    print(f"[INFO] 対象 sync ファイル: {len(sync_filepaths)} 件")
    if master_repo_name:
        print(f"[INFO] コピー元リポジトリ: {master_repo_name}")
    print()

    target_repos = collect_target_repos(siblings, PREREQUISITE)
    if not target_repos:
        print(f"[WARN] {PREREQUISITE} を持つリポジトリが見つからなかった。")
        sys.exit(1)

    print(f"[INFO] 処理対象リポジトリ: {len(target_repos)} 件")
    for d in target_repos:
        print(f"  {d.name}")
    print()

    uncommitted_map = show_remote_local_status(target_repos, sync_filepaths)

    if uncommitted_map:
        repo_names = [d.name for d in sorted(uncommitted_map.keys())]
        if not confirm_action(repo_names, "上記リポジトリのlocalの変更をremoteにcommit & pushしてよいか？"):
            print("[ABORT] キャンセルした。")
            sys.exit(0)
        print()
        print("=" * 60)
        print("[PHASE 1a] commit & push 実行")
        print("=" * 60)
        print()
        for repo_dir in sorted(uncommitted_map.keys()):
            filepaths = [fp for fp, _, _, _ in uncommitted_map[repo_dir]]
            commit_and_push_repo(repo_dir, filepaths)
        print("[OK] 未commit 分の同期が完了した。")
        print()

    print("=" * 60)
    print("[PHASE 2] local ハッシュ横断比較")
    print("=" * 60)
    print()

    copy_plan: dict[Path, list[tuple[Path, Path]]] = defaultdict(list)
    actual_master_repo_names: set[str] = set()

    for sync_filepath in sync_filepaths:
        majority_hash, outliers = detect_outliers(target_repos, sync_filepath)
        if not outliers:
            continue

        master_repo = find_master_repo(
            target_repos,
            sync_filepath,
            majority_hash,
            master_repo_name,
        )
        if master_repo is None:
            print(f"  [ERROR] {sync_filepath.as_posix()} : コピー元リポジトリが特定できなかった。")
            continue

        print(f"  コピー元: {master_repo.name}")
        actual_master_repo_names.add(master_repo.name)
        for repo_dir in outliers:
            local_fp = repo_dir / sync_filepath
            master_fp = master_repo / sync_filepath
            copy_plan[repo_dir].append((sync_filepath, master_fp))
            if local_fp.exists():
                print(f"  --- 差分内容 ({repo_dir.name}/{sync_filepath.as_posix()}: local → {master_repo.name}) ---")
                show_difft(local_fp, master_fp, label_a="local", label_b=master_repo.name)

    print()

    if not copy_plan:
        print("[OK] すべてのファイルが一致している。何もしない。")
        sys.exit(0)

    repo_names = [d.name for d in sorted(copy_plan.keys())]
    source_label = "、".join(sorted(actual_master_repo_names)) if actual_master_repo_names else (master_repo_name or "多数派")
    if not confirm_action(
        repo_names,
        f"上記リポジトリに {source_label} リポジトリのファイルをコピーして commit & push してよいか？",
    ):
        print("[ABORT] キャンセルした。")
        sys.exit(0)

    print()
    print("=" * 60)
    print("[PHASE 3] 同期実行")
    print("=" * 60)
    print()

    for repo_dir in sorted(copy_plan.keys()):
        sync_repo(repo_dir, copy_plan[repo_dir])

    print("[OK] すべての対象リポジトリへの同期が完了した。")


if __name__ == "__main__":
    main()
