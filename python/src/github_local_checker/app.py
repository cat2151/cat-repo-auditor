"""
github_local_checker/app.py

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
import sys
from datetime import datetime
from pathlib import Path

try:
    from .checker import check_repo
    from .config_loader import load_config
    from .constants import (
        DEFAULT_CONFIG,
        DEFAULT_OUTPUT,
        STATUS_DIVERGED,
        STATUS_LABEL,
        STATUS_PULLABLE,
        STATUS_UNKNOWN,
        STATUS_UP_TO_DATE,
    )
    from .display import colored
    from .git_utils import pull_repo
except ImportError:
    from checker import check_repo
    from config_loader import load_config
    from constants import (
        DEFAULT_CONFIG,
        DEFAULT_OUTPUT,
        STATUS_DIVERGED,
        STATUS_LABEL,
        STATUS_PULLABLE,
        STATUS_UNKNOWN,
        STATUS_UP_TO_DATE,
    )
    from display import colored
    from git_utils import pull_repo


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
