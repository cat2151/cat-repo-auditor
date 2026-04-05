# cat-repo-auditor

GitHubリポジトリのremote/local状況をlistし、可視化し、メンテの一部を自動化して効率化するTUI。Rustで書かれています。

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/cat2151/cat-repo-auditor)

## 状況

ドッグフーディング中です。

### install

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### 実行

```
catrepo
```

### update

```
catrepo update
```

もし失敗したら以下を実行（installコマンドと同じ）：

```
cargo install --force --git https://github.com/cat2151/cat-repo-auditor
```

### ビルド時に埋め込まれた commit hash を表示する:

```
catrepo hash
```

### 使い方

- 自分用なので他人が使うことを想定していません。以下も自分用のメモです。

- 作ったモチベ
  - 趣味OSSのリポジトリが増えてきました。整備に認知負荷をとられて手間が増えています。
  - そこでTUI。TUIを自作して整備を楽にします。
  - TUIは小規模ならClaude無料版chatで楽に作れるので、それでいきます。

- 使い方
  - config
    - 初回起動時に、config.toml が local config dirに生成され、そのfullpathが表示されます。
      - それをヒントにして、config.tomlを自力で編集してください。
      - 編集しないと動きません。
  - help
    - 起動したら、`?` キーを押すとhelpが出ます。

- PoC
  - リポジトリ全体の使い方としては、PoC。
  - これくらいの小規模なTUIなら、Claude無料版chatで作れます、を実証する用です。
  - なので、みんなも自分用に作ろう！というのを伝える用です。

## 以下はold。別物。python/ に退避済み。あとで書き直す予定です

## 概要

`gh` コマンド（GitHub CLI）で認証済みのユーザーのリポジトリを直近20件取得し、
各リポジトリに対して以下の項目を自動チェックする。結果は JSON ファイルに出力され、
カラー付きサマリーをターミナルに表示する。

### チェック項目

| 項目 | 説明 |
|------|------|
| `README.ja.md` | 日本語 README の存在 |
| DeepWiki 記載 | `README.ja.md` 内に DeepWiki へのリンクがあるか |
| `google*.html` | Google Search Console 用確認ファイルの存在 |
| `AGENTS.md` / `copilot-instructions.md` | AI エージェント向け指示ファイルの存在 |
| `.github/workflows/*.yml` | CI/CD ワークフローの存在 |
| `_config.yml` | Jekyll 設定ファイルの存在 |

## 必要環境

- Python 3.11 以上（または Python 3.10 以下 + `pip install tomli`）
- [GitHub CLI](https://cli.github.com/) がインストール済みで `gh auth login` 認証済みであること

## インストール

```bash
git clone https://github.com/cat2151/cat-repo-auditor.git
cd cat-repo-auditor
```

追加パッケージは不要（Python 3.11+ 標準ライブラリのみ使用）。

Python 3.10 以下の場合:

```bash
pip install tomli
```

## 設定

カレントディレクトリに `config.toml` を作成する。

```toml
github_user = "your-github-username"
```

## 使い方

```bash
python cat_repo_auditor.py
```

オプション:

```
--output, -o    JSON 出力ファイルパス（デフォルト: repo_analysis.json）
--config, -c    設定ファイルパス（デフォルト: config.toml）
```

## 出力例

ターミナルには Monokai カラーでサマリーが表示される。

```
=== GitHub リポジトリ分析CLI ===
実行日時: 2026-02-23 12:00:00
対象ユーザー: your-github-username
認証: gh auth token で取得済み

[1/3] your-github-username のリポジトリを取得中...
      20 件取得

[2/3] 各リポジトリを分析中...
  [ 1/20] my-project
         ✓ README.ja | ✗ DeepWiki | ✗ google | ✓ agents | ✓ CI | ✗ jekyll

[3/3] サマリー
======================================================================
  README.ja.md  [15/20 あり / 5/20 なし]
    ✗ some-repo
      https://github.com/your-github-username/some-repo
    ...
```

JSON ファイル（`repo_analysis.json`）には各リポジトリの詳細情報が含まれる。

## github_local_checker.py

- local側を軸にしたチェックツール
- 同じTOMLを利用する
- 通常実行するとdry-run的に、localリポジトリをチェックして結果をprintする
- `--pull` をつけて実行すると、pullableなものをすべてpullする
- 用途は、大量の実験用の小規模リポジトリを持っているuserが、把握を楽にするため、localに大量にpullする用

# check_local_workflows.py

- local側を軸にしたチェックツール
- 同じTOMLを利用する
- hashチェックする
- 用途は、大量の実験用の小規模リポジトリを持っているuserが、把握を楽にする用

# sync_workflows.py

- 用途は、localのリポジトリ間で、多数派のworkflowファイルに揃える用
- 最終的なcommit push前に、y/[N]で確認プロンプトが表示される
