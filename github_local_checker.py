#!/usr/bin/env python3
"""
github_local_checker.py

カレントディレクトリの1階層上にあるディレクトリ群を検索し、
config.toml で指定した GitHub ユーザーのローカルリポジトリを特定する。
各リポジトリについて状態を次の3分類で判定し、JSON 出力 + サマリ表示する。

  pullable  : dirty でなく、behind > 0、ahead == 0  → 今すぐ pull 可能
  diverged  : behind > 0 かつ ahead > 0              → 要注意（手動マージ等が必要）
  up_to_date: behind == 0                             → 最新（pull 不要）
  ※ dirty かつ behind > 0 かつ ahead == 0 の場合は unknown（pull したいが dirty で不可）

起動時は常に git fetch を実行する。

使い方:
    python github_local_checker.py [--config path/to/config.toml] [--output result.json] [--pull]
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path
from datetime import datetime

# tomllib は Python 3.11+ 標準。それ以前は tomli を使う。
try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib  # type: ignore
    except ImportError:
        print("ERROR: tomllib (Python 3.11+) または tomli パッケージが必要だ。")
        print("       pip install tomli  でインストールしてくれ。")
        sys.exit(1)


# ---------------------------------------------------------------------------
# 定数: status 値
# ---------------------------------------------------------------------------

STATUS_PULLABLE   = "pullable"    # 今すぐ pull 可能
STATUS_DIVERGED   = "diverged"    # behind かつ ahead（要注意）
STATUS_UP_TO_DATE = "up_to_date"  # 最新
STATUS_UNKNOWN    = "unknown"     # 判定不能（fetch 失敗・dirty で behind あり など）


# ---------------------------------------------------------------------------
# 設定読み込み
# ---------------------------------------------------------------------------

DEFAULT_CONFIG = "config.toml"
DEFAULT_OUTPUT = "github_local_checker_result.json"


def load_config(config_path: str) -> dict:
    path = Path(config_path)
    if not path.exists():
        print(f"ERROR: 設定ファイルが見つからない: {config_path}")
        sys.exit(1)
    with open(path, "rb") as f:
        return tomllib.load(f)


# ---------------------------------------------------------------------------
# Git ユーティリティ
# ---------------------------------------------------------------------------

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


# ---------------------------------------------------------------------------
# 3分類ロジック
# ---------------------------------------------------------------------------

def classify(dirty: bool, behind: int, ahead: int) -> str:
    """
    pullable  : not dirty, behind > 0, ahead == 0
    diverged  : behind > 0, ahead > 0  （dirty の有無に関わらず diverged を優先表示）
    up_to_date: behind == 0
    unknown   : 上記以外（取得不能・dirty で behind あり など）
    """
    if behind < 0 or ahead < 0:
        return STATUS_UNKNOWN
    if behind > 0 and ahead > 0:
        return STATUS_DIVERGED
    if behind == 0:
        return STATUS_UP_TO_DATE
    # behind > 0, ahead == 0 のケース
    if not dirty:
        return STATUS_PULLABLE
    # dirty かつ behind > 0 → pull したいが今は不可
    return STATUS_UNKNOWN


# ---------------------------------------------------------------------------
# 1 リポジトリのチェック
# ---------------------------------------------------------------------------

def check_repo(path: str, github_username: str) -> dict:
    """
    リポジトリを解析して結果 dict を返す。

    {
        "path"       : str,
        "name"       : str,
        "is_target"  : bool,
        "remote_url" : str | null,
        "branch"     : str | null,
        "dirty"      : bool | null,
        "behind"     : int | null,
        "ahead"      : int | null,
        "status"     : "pullable" | "diverged" | "up_to_date" | "unknown" | null,
        "error"      : str | null,
    }
    """
    result: dict = {
        "path"      : path,
        "name"      : Path(path).name,
        "is_target" : False,
        "remote_url": None,
        "branch"    : None,
        "dirty"     : None,
        "behind"    : None,
        "ahead"     : None,
        "status"    : None,
        "error"     : None,
    }
    errors: list[str] = []

    # --- git リポジトリか ---
    if not is_git_repo(path):
        result["error"] = "git リポジトリではない"
        return result

    # --- remote URL 取得 ---
    remote_url = get_remote_url(path)
    result["remote_url"] = remote_url
    if remote_url is None:
        result["error"] = "origin が設定されていない"
        return result

    # --- 対象ユーザーか ---
    if not is_target_repo(remote_url, github_username):
        return result  # is_target = False のまま返す

    result["is_target"] = True

    # --- ブランチ取得 ---
    branch = get_current_branch(path)
    result["branch"] = branch
    if branch is None or branch == "HEAD":
        result["error"] = "detached HEAD 状態か、ブランチ名取得失敗"
        result["status"] = STATUS_UNKNOWN
        return result

    # --- dirty チェック ---
    dirty = is_dirty(path)
    result["dirty"] = dirty

    # --- fetch（常に実行） ---
    fetch_ok, fetch_err = fetch_remote(path)
    if not fetch_ok:
        errors.append(fetch_err or "git fetch 失敗")

    # --- behind / ahead ---
    behind, ahead = get_behind_ahead(path, branch)
    if behind >= 0:
        result["behind"] = behind
        result["ahead"]  = ahead
    else:
        errors.append("tracking ブランチが見つからない（origin に対応ブランチがないかもしれない）")

    # --- 3分類 ---
    result["status"] = classify(dirty, behind, ahead)

    if errors:
        result["error"] = " / ".join(errors)

    return result


# ---------------------------------------------------------------------------
# 表示ユーティリティ
# ---------------------------------------------------------------------------

STATUS_LABEL = {
    STATUS_PULLABLE  : "PULLABLE   ✓",
    STATUS_DIVERGED  : "DIVERGED   ⚠",
    STATUS_UP_TO_DATE: "UP-TO-DATE  ",
    STATUS_UNKNOWN   : "UNKNOWN    ?",
}

_COLOR = {
    STATUS_PULLABLE  : "\033[32m",   # 緑
    STATUS_DIVERGED  : "\033[33m",   # 黄
    STATUS_UP_TO_DATE: "\033[0m",    # デフォルト
    STATUS_UNKNOWN   : "\033[31m",   # 赤
}
_RESET = "\033[0m"


def colored(text: str, status: str) -> str:
    return f"{_COLOR.get(status, '')}{text}{_RESET}"


# ---------------------------------------------------------------------------
# メイン処理
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="GitHub ローカルリポジトリの状態を一括チェックする（fetch 常時実行）"
    )
    parser.add_argument(
        "--config", default=DEFAULT_CONFIG,
        help=f"設定ファイルパス (デフォルト: {DEFAULT_CONFIG})"
    )
    parser.add_argument(
        "--output", default=DEFAULT_OUTPUT,
        help=f"JSON 出力ファイルパス (デフォルト: {DEFAULT_OUTPUT})"
    )
    parser.add_argument(
        "--pull", action="store_true",
        help="pullable なリポジトリを実際に pull する"
    )
    args = parser.parse_args()

    # --- 設定読み込み ---
    config = load_config(args.config)
    github_username: str = config.get("github_user", "")
    if not github_username:
        print("ERROR: config.toml に github_user が設定されていない。")
        sys.exit(1)

    # --- 対象ディレクトリ列挙 ---
    current_dir = Path.cwd()
    parent_dir  = current_dir.parent
    siblings = [
        str(d)
        for d in sorted(parent_dir.iterdir())
        if d.is_dir() and d != current_dir
    ]

    print(f"GitHub ユーザー         : {github_username}")
    print(f"スキャン元ディレクトリ  : {parent_dir}")
    print(f"スキャン対象数          : {len(siblings)}")
    print("-" * 64)

    # --- 各ディレクトリをチェック ---
    results = []
    for d in siblings:
        r = check_repo(d, github_username)
        results.append(r)

        if not r["is_target"]:
            print(f"  [SKIP]                   {r['name']}")
            continue

        status = r["status"] or STATUS_UNKNOWN
        label  = STATUS_LABEL.get(status, "?")

        detail_parts = []
        if r["dirty"]:
            detail_parts.append("dirty")
        if r["behind"] is not None and r["behind"] > 0:
            detail_parts.append(f"behind {r['behind']}")
        if r["ahead"] is not None and r["ahead"] > 0:
            detail_parts.append(f"ahead {r['ahead']}")
        detail = ", ".join(detail_parts) if detail_parts else "clean"

        err_str = f"  ⚠ {r['error']}" if r["error"] else ""

        print(f"  [{colored(label, status)}]  {r['name']}  ({detail}){err_str}")

    print("-" * 64)

    # --- 集計 ---
    target_repos  = [r for r in results if r["is_target"]]
    pullable_repos = [r for r in target_repos if r["status"] == STATUS_PULLABLE]
    diverged_repos = [r for r in target_repos if r["status"] == STATUS_DIVERGED]
    uptodate_repos = [r for r in target_repos if r["status"] == STATUS_UP_TO_DATE]
    unknown_repos  = [r for r in target_repos if r["status"] == STATUS_UNKNOWN]

    print("\n=== サマリ ===")
    print(f"  スキャンしたディレクトリ : {len(results)}")
    print(f"  対象リポジトリ           : {len(target_repos)}")
    print(colored(f"  pull 可能   (pullable)   : {len(pullable_repos)}", STATUS_PULLABLE))
    print(colored(f"  要注意      (diverged)   : {len(diverged_repos)}", STATUS_DIVERGED))
    print(f"  最新        (up_to_date) : {len(uptodate_repos)}")
    if unknown_repos:
        print(colored(f"  判定不能    (unknown)    : {len(unknown_repos)}", STATUS_UNKNOWN))

    if pullable_repos:
        print("\n  今すぐ pull 可能:")
        for r in pullable_repos:
            print(f"    - {r['name']}  (behind {r['behind']})")

    if diverged_repos:
        print("\n  diverged（要注意）:")
        for r in diverged_repos:
            print(f"    - {r['name']}  (behind {r['behind']}, ahead {r['ahead']})")

    # --- --pull 実行 ---
    pull_results: dict[str, dict] = {}  # name -> {success, message}
    if args.pull:
        if pullable_repos:
            print("\n=== --pull 実行 ===")
            for r in pullable_repos:
                ok, msg = pull_repo(r["path"])
                pull_results[r["name"]] = {"success": ok, "message": msg}
                icon = "✓" if ok else "✗"
                label_color = STATUS_PULLABLE if ok else STATUS_UNKNOWN
                print(f"  [{colored(icon, label_color)}] {r['name']}: {msg}")
        else:
            print("\n  --pull: pull 対象なし（pullable なリポジトリがない）")


    # --- JSON 出力 ---
    output_data = {
        "generated_at"   : datetime.now().isoformat(),
        "github_username": github_username,
        "scanned_from"   : str(parent_dir),
        "do_pull"        : args.pull,
        "summary": {
            "total_scanned": len(results),
            "target_repos" : len(target_repos),
            "pullable"     : len(pullable_repos),
            "diverged"     : len(diverged_repos),
            "up_to_date"   : len(uptodate_repos),
            "unknown"      : len(unknown_repos),
        },
        "pull_results" : pull_results,
        "repositories" : results,
    }

    json_str = json.dumps(output_data, ensure_ascii=False, indent=2)

    Path(args.output).write_text(json_str, encoding="utf-8")
    print(f"\nJSON を保存した: {args.output}")


if __name__ == "__main__":
    main()
