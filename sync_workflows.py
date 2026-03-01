#!/usr/bin/env python3
"""
sync_workflows.py

README.ja.md を持つ git リポジトリを対象に、
config.toml の sync_filepaths に列挙されたファイルについて
  - 欠落している
  - コピー元（master_repo または多数派）と異なる
のいずれかであれば、コピー元ファイルをコピーして commit & push する。

差分は difft（利用可能な場合）または diff で可視化する。

実施前に y[/n] で確認プロンプトが表示される。

config.toml の設定例:
    [sync]
    master_repo = "github-actions"   # コピー元リポジトリ名（省略時は多数派を使用）
    sync_filepaths = [
        ".github/workflows/call-check-large-files.yml",
    ]

使い方:
    python sync_workflows.py
"""

import sys
import hashlib
import shutil
import subprocess
import tempfile
import tomllib
from collections import Counter, defaultdict
from pathlib import Path


CONFIG_FILE  = Path(__file__).parent / "config.toml"
COMMIT_MSG   = "chore: sync files to match majority"
PREREQUISITE = "README.ja.md"   # 処理対象の前提条件ファイル


# ---------------------------------------------------------------------------
# 設定読み込み
# ---------------------------------------------------------------------------

def load_sync_config() -> tuple[list[Path], str | None]:
    """config.toml から sync_filepaths と master_repo を読み込む。

    Returns:
        (sync_filepaths, master_repo): sync_filepaths は同期対象ファイルのリスト、
        master_repo はコピー元リポジトリ名 (設定なしの場合は None)。
    """
    if not CONFIG_FILE.exists():
        print(f"[ERROR] config.toml が見つからない: {CONFIG_FILE}")
        sys.exit(1)
    with open(CONFIG_FILE, "rb") as f:
        config = tomllib.load(f)
    sync = config.get("sync", {})
    paths = sync.get("sync_filepaths", [])
    if not paths:
        print("[ERROR] config.toml に sync_filepaths が設定されていない。")
        sys.exit(1)
    return [Path(p) for p in paths], sync.get("master_repo", None)


# ---------------------------------------------------------------------------
# 対象リポジトリ判定
# ---------------------------------------------------------------------------

def is_git_repo(path: Path) -> bool:
    """指定パスが git リポジトリかどうかを返す。"""
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
# git ユーティリティ
# ---------------------------------------------------------------------------

def git_run(args: list[str], cwd: Path) -> subprocess.CompletedProcess:
    """git コマンドを実行し、失敗時はエラーを表示して終了する。"""
    cmd = ["git"] + args
    result = subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True)
    if result.returncode != 0:
        print(f"[ERROR] コマンド失敗: {' '.join(cmd)}")
        if result.stderr.strip():
            print(result.stderr.strip())
        if result.stdout.strip():
            print(result.stdout.strip())
        sys.exit(result.returncode)
    return result


def git_fetch(repo_dir: Path) -> None:
    """origin を fetch する。"""
    git_run(["-C", str(repo_dir), "fetch", "origin"], cwd=repo_dir)


def git_show_remote_file(repo_dir: Path, filepath: Path) -> bytes | None:
    """remote HEAD のファイル内容をバイト列で返す。取得失敗時は None。"""
    result = subprocess.run(
        ["git", "-C", str(repo_dir), "show", f"origin/HEAD:{filepath.as_posix()}"],
        capture_output=True,
    )
    return result.stdout if result.returncode == 0 else None


def git_add(filepath: Path, repo_dir: Path) -> None:
    """ファイルを git add する。"""
    git_run(["-C", str(repo_dir), "add", filepath.as_posix()], cwd=repo_dir)


def git_has_staged_changes(repo_dir: Path) -> bool:
    """ステージに差分があれば True を返す。"""
    result = subprocess.run(
        ["git", "-C", str(repo_dir), "diff", "--cached", "--quiet"],
        capture_output=True,
    )
    return result.returncode == 1


def git_commit(message: str, repo_dir: Path) -> None:
    """git commit する。"""
    git_run(["-C", str(repo_dir), "commit", "-m", message], cwd=repo_dir)


def git_push(repo_dir: Path) -> None:
    """git push する。"""
    git_run(["-C", str(repo_dir), "push"], cwd=repo_dir)


# ---------------------------------------------------------------------------
# ファイルユーティリティ
# ---------------------------------------------------------------------------

def file_sha256(path: Path) -> str:
    """ファイルの SHA-256 ハッシュを返す（バイト列ベース）。"""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bytes_sha256(data: bytes) -> str:
    """バイト列の SHA-256 ハッシュを返す。"""
    return hashlib.sha256(data).hexdigest()


def copy_file(src: Path, dst: Path) -> None:
    """ファイルを上書きコピーする。親ディレクトリがなければ作成する。"""
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(str(src), str(dst))


# ---------------------------------------------------------------------------
# 多数派ハッシュの算出
# ---------------------------------------------------------------------------

def compute_majority_hash(
    target_repos: list[Path],
    sync_filepath: Path,
) -> str | None:
    """
    sync_filepath を持つリポジトリのハッシュから多数派を返す。
    1件も存在しなければ None を返す。
    """
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
    """コピー元リポジトリを返す。

    master_repo_name が指定されている場合はその名前のリポジトリを優先する。
    見つからない場合は多数派ハッシュを持つ最初のリポジトリを返す。
    """
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
        (d for d in target_repos
         if (d / sync_filepath).exists()
         and file_sha256(d / sync_filepath) == majority_hash),
        None,
    )


# ---------------------------------------------------------------------------
# 差分の可視化
# ---------------------------------------------------------------------------

def show_difft(path_a: Path, path_b: Path) -> None:
    """difft を使って2ファイルの差分を表示する。difft がなければ diff にフォールバック。

    Args:
        path_a: 比較元ファイル (旧/remote など)。
        path_b: 比較先ファイル (新/local など)。
    """
    for cmd in (
        ["difft", str(path_a), str(path_b)],
        ["diff", "-u", str(path_a), str(path_b)],
    ):
        try:
            completed = subprocess.run(cmd, check=False)
        except FileNotFoundError:
            continue
        # difft: treat any non-zero exit code as an error and fall back
        if cmd[0] == "difft":
            if completed.returncode == 0:
                return
            continue
        # diff: 0 = no differences, 1 = files differ, >1 = error
        if cmd[0] == "diff":
            if completed.returncode in (0, 1):
                return
            continue
    print("  [INFO] difft/diff コマンドが見つからない、または実行に失敗したため、内容比較をスキップする。")


# ---------------------------------------------------------------------------
# remote vs local 差分チェック（autocrlf 迂回のためバイト比較）
# ---------------------------------------------------------------------------

def get_uncommitted_vs_remote(
    repo_dir: Path,
    sync_filepaths: list[Path],
) -> list[tuple[Path, str, str, bytes]]:
    """
    worktree のファイルと remote HEAD をバイト比較し、
    差異があるものを (filepath, local_hash, remote_hash, remote_bytes) で返す。
    ファイルが local に存在しない場合は対象外（Phase 2 が担当）。
    """
    results = []
    for fp in sync_filepaths:
        local_path   = repo_dir / fp
        if not local_path.exists():
            continue  # 欠落は Phase 2 のコピー処理が担当
        remote_bytes = git_show_remote_file(repo_dir, fp)
        if remote_bytes is None:
            continue  # remote にも存在しない場合はスキップ
        local_hash  = file_sha256(local_path)
        remote_hash = bytes_sha256(remote_bytes)
        if local_hash != remote_hash:
            results.append((fp, local_hash, remote_hash, remote_bytes))
    return results


# ---------------------------------------------------------------------------
# Phase 1: remote vs local 差分の可視化
# ---------------------------------------------------------------------------

def show_remote_local_status(
    target_repos: list[Path],
    sync_filepaths: list[Path],
) -> dict[Path, list[tuple[Path, str, str, bytes]]]:
    """
    対象リポジトリを fetch し、remote/local の差分状態を表示する。
    未commit の差分がある repo_dir -> [(filepath, local_hash, remote_hash, remote_bytes)] を返す。
    """
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
            print(f"    [OK] 差分なし")
            continue

        for fp, lh, rh, remote_bytes in diffs:
            print(f"    [!] 差分あり: {fp.as_posix()}")
            print(f"        local  : {lh}")
            print(f"        remote : {rh}")
            print(f"        ※ localの変更をremoteにcommit & pushする予定")
            with tempfile.NamedTemporaryFile(
                suffix=fp.suffix or ".txt", delete=False, prefix="remote_"
            ) as tmp:
                tmp.write(remote_bytes)
                tmp_path = Path(tmp.name)
            try:
                print(f"        --- 差分内容 (remote → local) ---")
                show_difft(tmp_path, repo_dir / fp)
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


# ---------------------------------------------------------------------------
# Phase 2: local ハッシュ横断比較（欠落 or 多数派と不一致）
# ---------------------------------------------------------------------------

def detect_outliers(
    target_repos: list[Path],
    sync_filepath: Path,
) -> tuple[str | None, list[Path]]:
    """
    sync_filepath について多数派ハッシュを算出し、
    欠落または不一致のリポジトリ一覧を返す。
    戻り値: (majority_hash, outlier_repos)
    """
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


# ---------------------------------------------------------------------------
# 確認プロンプト
# ---------------------------------------------------------------------------

def confirm_action(repo_names: list[str], action: str) -> bool:
    """対象リポジトリ名と操作内容を表示し、確認を求める。y なら True を返す。"""
    print(f"対象リポジトリ: {len(repo_names)} 件")
    for name in repo_names:
        print(f"  {name}")
    print()
    answer = input(f"{action} [y/N]: ")
    return answer.strip().lower() == "y"


# ---------------------------------------------------------------------------
# 同期実行
# ---------------------------------------------------------------------------

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
    print(f"  [PUSH] 完了")
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
    print(f"  [PUSH] 完了")
    print()


# ---------------------------------------------------------------------------
# エントリーポイント
# ---------------------------------------------------------------------------

def main() -> None:
    sync_filepaths, master_repo_name = load_sync_config()
    parent         = Path.cwd().parent
    siblings       = sorted(p for p in parent.iterdir() if p.is_dir())

    print(f"[INFO] 親ディレクトリ    : {parent}")
    print(f"[INFO] 前提条件ファイル  : {PREREQUISITE}")
    print(f"[INFO] 対象 sync ファイル: {len(sync_filepaths)} 件")
    if master_repo_name:
        print(f"[INFO] コピー元リポジトリ: {master_repo_name}")
    print()

    target_repos = collect_target_repos(siblings)
    if not target_repos:
        print(f"[WARN] {PREREQUISITE} を持つリポジトリが見つからなかった。")
        sys.exit(1)

    print(f"[INFO] 処理対象リポジトリ: {len(target_repos)} 件")
    for d in target_repos:
        print(f"  {d.name}")
    print()

    # Phase 1: remote vs local 差分を可視化
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

    # Phase 2: local ハッシュ横断比較
    print("=" * 60)
    print("[PHASE 2] local ハッシュ横断比較")
    print("=" * 60)
    print()

    # repo_dir -> [(sync_filepath, master_fp)]
    copy_plan: dict[Path, list[tuple[Path, Path]]] = defaultdict(list)
    actual_master_repo_names: set[str] = set()

    for sync_filepath in sync_filepaths:
        majority_hash, outliers = detect_outliers(target_repos, sync_filepath)
        if not outliers:
            continue
        master_repo = find_master_repo(target_repos, sync_filepath, majority_hash, master_repo_name)
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
                show_difft(local_fp, master_fp)

    print()

    if not copy_plan:
        print("[OK] すべてのファイルが一致している。何もしない。")
        sys.exit(0)

    repo_names = [d.name for d in sorted(copy_plan.keys())]
    source_label = "、".join(sorted(actual_master_repo_names)) if actual_master_repo_names else (master_repo_name or "多数派")
    if not confirm_action(
        repo_names,
        f"上記リポジトリに {source_label} リポジトリのファイルをコピーして commit & push してよいか？"
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
