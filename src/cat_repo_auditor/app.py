#!/usr/bin/env python3
"""
GitHubリポジトリを分析するCLI

認証: gh コマンド (GitHub CLI) が認証済みであること。
      `gh auth token` でトークンを自動取得するため --token 指定は不要。

チェック項目:
  - README.ja.md の存在
  - README.ja.md 内の DeepWiki 記載有無
  - google*.html がプロジェクトルートに存在するか
  - AGENTS.md または .github/copilot-instructions.md のどちらかが存在するか
  - .github/workflows/ 配下に yml/yaml が存在するか
  - プロジェクトルートに _config.yml (Jekyll用) が存在するか
"""

import json
import sys
import argparse
from datetime import datetime
from pathlib import Path

from .colors import C, ok, ng, head, dim
from .config_loader import load_config
from .github_api import get_token_from_gh, github_request, fetch_root_listing
from .cache import (
    load_history, save_history, is_cache_from_today,
    load_repo_cache, save_repo_cache,
    load_known_repo_names, append_repos_to_config, print_repo_config,
)
from .checkers import (
    fetch_readme_ja, check_deepwiki, analyze_readme,
    check_google_html, check_agents_file, check_workflows, check_jekyll_config,
)

# GITHUB_USER は config.toml から取得 (load_config() 参照)

# ---------------------------------------------------------------------------
# リポジトリ一覧
# ---------------------------------------------------------------------------

def fetch_repos(token, github_user):
    history = load_history()
    if is_cache_from_today(history):
        cached = load_repo_cache()
        if cached is not None:
            print(f"{C.ORANGE}[1/3]{C.RESET} {github_user} のリポジトリをキャッシュから取得...")
            print(f"      {len(cached)} 件 (cache/repositories.json)")
            return cached
    print(f"{C.ORANGE}[1/3]{C.RESET} {github_user} のリポジトリを取得中...")
    url = (
        f"https://api.github.com/users/{github_user}/repos"
        f"?sort=pushed&direction=desc&per_page=20"
    )
    repos = github_request(url, token)
    if not repos:
        print(f"{C.NG_RED}ERROR{C.RESET}: リポジトリの取得に失敗した", file=sys.stderr)
        sys.exit(1)
    print(f"      {len(repos)} 件取得")
    save_repo_cache(repos)
    save_history()
    return repos


# ---------------------------------------------------------------------------
# メイン処理
# ---------------------------------------------------------------------------

def process_repos(repos, token, github_user):
    print(f"\n{C.ORANGE}[2/3]{C.RESET} 各リポジトリを分析中...")
    results = []

    for i, repo in enumerate(repos, 1):
        name = repo["name"]
        print(f"  [{i:2d}/{len(repos)}] {C.REPO}{name}{C.RESET}", flush=True)

        result = {
            "repo_name": name,
            "full_name": repo["full_name"],
            "html_url": repo["html_url"],
            "description": repo.get("description"),
            "pushed_at": repo.get("pushed_at"),
            "created_at": repo.get("created_at"),
            "stars": repo.get("stargazers_count", 0),
            "language": repo.get("language"),
            "is_fork": repo.get("fork", False),
            "is_archived": repo.get("archived", False),
            "readme_ja_exists": False,
            "readme_ja_analysis": None,
            "deepwiki": None,
            "google_html": None,
            "agents_file": None,
            "workflows_yml": None,
            "jekyll_config": None,
        }

        root_files = fetch_root_listing(name, token, github_user)
        readme_content = fetch_readme_ja(name, token, github_user)

        if readme_content:
            result["readme_ja_exists"] = True
            result["readme_ja_analysis"] = analyze_readme(readme_content)
            result["deepwiki"] = check_deepwiki(readme_content)
        else:
            result["deepwiki"] = {"has_deepwiki": False, "matched_patterns": [], "occurrences": []}

        result["google_html"]   = check_google_html(root_files)
        result["agents_file"]   = check_agents_file(name, root_files, token, github_user)
        result["workflows_yml"] = check_workflows(name, token, github_user)
        result["jekyll_config"] = check_jekyll_config(root_files)

        def flag(label, val):
            return (f"{C.OK_GRN}\u2713{C.RESET} {C.FG}{label}{C.RESET}") if val else (f"{C.NG_RED}\u2717{C.RESET} {C.DIM}{label}{C.RESET}")

        flags = [
            flag("README.ja", result["readme_ja_exists"]),
            flag("DeepWiki",  result["deepwiki"]["has_deepwiki"]),
            flag("google",    result["google_html"]["exists"]),
            flag("agents",    result["agents_file"]["exists"]),
            flag("CI",        result["workflows_yml"]["exists"]),
            flag("jekyll",    result["jekyll_config"]["exists"]),
        ]
        print(f"         {' | '.join(flags)}")

        results.append(result)

    return results


def print_summary(results, output_path):
    W = 70
    print(f"\n{C.ORANGE}[3/3]{C.RESET} {C.FG}{C.BOLD}サマリー{C.RESET}")
    print("=" * W)

    total = len(results)
    with_readme   = [r for r in results if r["readme_ja_exists"]]
    with_deepwiki = [r for r in results if r["deepwiki"]["has_deepwiki"]]


    def missing(key):
        return [{'repo_name': r['repo_name'], 'html_url': r['html_url']}
                for r in results if not (r.get(key) or {}).get('exists', False)]

    no_readme   = [{'repo_name': r['repo_name'], 'html_url': r['html_url']} for r in results if not r['readme_ja_exists']]
    no_deepwiki = [{'repo_name': r['repo_name'], 'html_url': r['html_url']} for r in results if not r['deepwiki']['has_deepwiki']]
    no_google   = missing('google_html')
    no_agents   = missing('agents_file')
    no_ci       = missing('workflows_yml')
    no_jekyll   = missing('jekyll_config')


    # no_list: [{'repo_name': ..., 'html_url': ...}, ...]
    def section(title, no_list, ok_count):
        bar = "\u2500" * W
        print(f"\n{bar}")
        label_ok = ok(f"{ok_count}/{total} あり")
        label_ng = ng(f"{len(no_list)}/{total} なし")
        print(f"  {head(title)}  [{label_ok} / {label_ng}]")
        if no_list:
            for item in no_list:
                print(f"    {C.NG_RED}\u2717{C.RESET} {C.REPO}{item['repo_name']}{C.RESET}")
                print(f"      {C.DIM}{item['html_url']}{C.RESET}")
        else:
            print(f"    {ok('(全リポジトリに存在する)')}")

    print(f"{C.FG}対象リポジトリ数: {C.PURPLE}{C.BOLD}{total}{C.RESET}")
    print(f"{C.DIM}フォーク: {sum(1 for r in results if r['is_fork'])}  "
          f"アーカイブ済: {sum(1 for r in results if r['is_archived'])}{C.RESET}")

    section("README.ja.md",                        no_readme,   len(with_readme))
    section("DeepWiki \u8a18\u8f09 (README.ja.md \u5185)",  no_deepwiki, len(with_deepwiki))
    section("google*.html (\u30eb\u30fc\u30c8)",             no_google,   total - len(no_google))
    section("AGENTS.md / copilot-instructions.md", no_agents,   total - len(no_agents))
    section(".github/workflows/ \u306e yml/yaml",  no_ci,       total - len(no_ci))
    section("_config.yml (Jekyll, \u30eb\u30fc\u30c8)",      no_jekyll,   total - len(no_jekyll))

    if no_deepwiki:
        print(f"\n{chr(9472)*W}")
        print(f"  {head('DeepWiki 記載なし 詳細:')}")
        for item in no_deepwiki:
            print(f"    {C.NG_RED}\u2717{C.RESET} {C.REPO}{item['repo_name']}{C.RESET}")
            print(f"      {C.DIM}{item['html_url']}{C.RESET}")

    print(f"\n{'='*W}")
    print(f"{C.DIM}出力ファイル: {output_path}{C.RESET}")
    print("=" * W)


# ---------------------------------------------------------------------------
# エントリポイント
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="GitHubリポジトリを多角的に分析するCLI (gh認証使用)"
    )
    parser.add_argument(
        "--output", "-o",
        help="JSON出力ファイルパス (デフォルト: repo_analysis.json)",
        default="repo_analysis.json",
    )
    parser.add_argument(
        "--config", "-c",
        help="設定ファイルパス (デフォルト: config.toml)",
        default="config.toml",
    )
    args = parser.parse_args()

    cfg = load_config(args.config)
    github_user = cfg["github_user"]

    print(f"{C.TITLE}{C.BOLD}=== GitHub リポジトリ分析CLI ==={C.RESET}")
    print(f"{C.DIM}実行日時: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}{C.RESET}")
    print(f"{C.DIM}対象ユーザー:{C.RESET} {C.REPO}{github_user}{C.RESET}")

    print_repo_config()
    print()

    token = get_token_from_gh()
    print(f"{C.DIM}認証:{C.RESET} {ok('gh auth token で取得済み')}")
    print()

    repos   = fetch_repos(token, github_user)

    # 新規リポジトリを config/repositories.toml に追記する
    repo_names   = [r["name"] for r in repos]
    known_names  = load_known_repo_names()
    new_names    = [n for n in repo_names if n not in known_names]
    if new_names:
        append_repos_to_config(new_names)
        print(f"{C.ORANGE}新規リポジトリを config/repositories.toml に追記した:{C.RESET} {', '.join(new_names)}")

    results = process_repos(repos, token, github_user)

    output = {
        "generated_at": datetime.now().isoformat(),
        "user": github_user,
        "total_repos": len(results),
        "repos": results,
    }

    output_path = Path(args.output)
    output_path.write_text(
        json.dumps(output, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    print_summary(results, str(output_path))


if __name__ == "__main__":
    main()
