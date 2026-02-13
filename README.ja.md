# cat-repo-auditor

# 以下はAIが生成した構想であり、現実とは異なります。今後修正していきます。

GitHubリポジトリ群の標準化を可視化・管理するツール

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Python: 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)

## 概要

`cat-repo-auditor`は、GitHubユーザーの複数リポジトリを監査し、標準化されたファイル構成の遵守状況を可視化するツールです。

### 主な特徴

- 📊 **リポジトリ群の可視化** - 複数リポジトリの標準遵守状況を一覧表示
- ⚙️ **TOML設定駆動** - チェック項目を宣言的に定義
- 🔥 **ホットリロード** - 設定ファイルの変更を自動検知して即座に反映
- 💾 **インテリジェントキャッシュ** - API呼び出しを最小化し、高速動作
- 🎨 **直感的なGUI** - Tkinterベースのシンプルなインターフェース
- 🆓 **完全無料** - 外部サービス不要、ローカルで完結

### ユースケース

- **個人開発者**: 複数のPoCリポジトリの標準化を維持
- **チーム開発**: 組織内リポジトリの品質ガイドライン遵守を確認
- **OSS管理**: 複数のOSSプロジェクトの一貫性を保つ
- **テンプレート検証**: リポジトリテンプレートの有効性を評価

## インストール

### 前提条件

- Python 3.10以上
- pip（Pythonパッケージマネージャー）
- Tkinter（通常はPythonに同梱）

### インストール手順

```bash
# リポジトリをクローン
git clone https://github.com/YOUR_USERNAME/cat-repo-auditor.git
cd cat-repo-auditor

# 依存関係をインストール
pip install -r requirements.txt

# GitHub Personal Access Tokenを設定（推奨）
export GITHUB_TOKEN=your_github_token_here
```

## 使い方

### 基本的な起動

```bash
python repo_auditor.py
```

または起動スクリプトを使用：

```bash
./start.sh
```

### 初回起動時の挙動

1. アプリケーションが起動
2. `audit_config.toml`が存在しない場合、デフォルト設定で自動生成
3. 指定したGitHubユーザーの直近20リポジトリを取得
4. 各リポジトリのファイル存在状況をチェック
5. 結果をテーブル形式で表示

### 画面の見方

```
┌─────────────────────────────────────────────────────────────┐
│ Repository        │ README │ AGENTS │ .gitignore │ Updated │
├─────────────────────────────────────────────────────────────┤
│ latest-project    │   ✓    │   ✓    │     ✓      │ 2025-02 │ ← 最新（青色背景）
│ older-project-1   │   ✓    │   ✗    │     ✓      │ 2025-01 │ ← 欠落あり（赤色背景）
│ older-project-2   │   ✓    │   ✓    │     ✓      │ 2024-12 │
└─────────────────────────────────────────────────────────────┘
```

- **青色の行**: 最新リポジトリ（比較基準）
- **赤色のセル**: 最新リポジトリには存在するが、当該リポジトリには欠落している項目
- **✓**: ファイルが存在
- **✗**: ファイルが存在しない

### 設定のカスタマイズ

`audit_config.toml`を編集することで、チェック項目や表示設定をカスタマイズできます：

```toml
# チェックするファイル/ディレクトリのリスト
check_items = [
    "README.md",
    "LICENSE",
    ".gitignore",
    "CONTRIBUTING.md",
    ".github/workflows/ci.yml",
    "pyproject.toml",
    "Dockerfile",
]

# 表示設定
[display]
show_repo_name = true        # リポジトリ名を表示
show_updated_at = true       # 更新日時を表示
highlight_missing = true     # 欠落項目を赤色で強調
```

設定ファイルを保存すると、**アプリケーションを再起動せずに自動的に反映**されます。

## アーキテクチャ

### ファイル構成

```
cat-repo-auditor/
├── repo_auditor.py         # メインアプリケーション
├── audit_config.toml       # 設定ファイル
├── requirements.txt        # Python依存関係
├── start.sh               # 起動スクリプト
├── .cache/                # キャッシュディレクトリ（自動生成）
│   ├── repos.json         # リポジトリ一覧キャッシュ
│   └── <repo_name>.json   # 個別リポジトリのチェック結果
├── README.md              # 英語版README
├── README.ja.md           # 日本語版README（このファイル）
└── LICENSE                # MITライセンス
```

### 主要コンポーネント

#### 1. RepoAuditor クラス

GitHubリポジトリの取得と監査を担当。

**主要メソッド:**
- `fetch_repositories(count: int)` - 指定ユーザーの直近リポジトリを取得
- `fetch_repo_details(repo_name: str, check_items: List[str])` - 個別リポジトリのファイル存在確認

**キャッシュ戦略:**
- リポジトリ一覧: 1時間有効の時限キャッシュ
- ファイル存在確認: 永続キャッシュ（設定変更時に差分取得）

#### 2. ConfigWatcher クラス

TOMLファイルの変更を監視し、ホットリロードを実現。

**動作原理:**
1. 1秒ごとにファイルのmtime（更新時刻）を確認
2. 変更を検知したらコールバック関数を実行
3. バックグラウンドスレッドで動作（UIをブロックしない）

#### 3. RepoAuditorGUI クラス

Tkinterベースのグラフィカルユーザーインターフェース。

**主要機能:**
- Treeviewウィジェットによるテーブル表示
- 非同期でのリポジトリ取得（UIフリーズを防止）
- 動的な列生成（TOML設定に基づく）
- カラーコーディングによる視覚的フィードバック

### データフロー

```
[GitHub API]
     ↓
[RepoAuditor] ←→ [キャッシュ]
     ↓
[RepoAuditorGUI]
     ↓
[ユーザー]
     ↓
[audit_config.toml] → [ConfigWatcher] → [ホットリロード]
```

## 開発ガイド（Coding Agent向け）

このセクションは、GitHub Copilot、Cursor、Windsurf、ClaudeなどのCoding Agentが本プロジェクトをゼロから実装する際のガイドラインです。

### 実装の前提条件

- Python 3.10以上を使用
- 標準ライブラリ優先（外部依存は最小限に）
- クロスプラットフォーム対応（Windows/Mac/Linux）
- PEP 8コーディング規約に準拠

### 実装ステップ

#### Step 1: プロジェクト構造の作成

```bash
mkdir cat-repo-auditor
cd cat-repo-auditor
touch repo_auditor.py audit_config.toml requirements.txt
```

#### Step 2: 依存関係の定義（requirements.txt）

```
requests>=2.31.0
# Note: tomllib is built into Python 3.11+
# For Python 3.10, install: tomli>=2.0.0
```

#### Step 3: 設定ファイルの実装（audit_config.toml）

```toml
# デフォルト設定の例
check_items = [
    "README.md",
    ".gitignore",
    "LICENSE",
]

[display]
show_repo_name = true
show_updated_at = true
highlight_missing = true
```

#### Step 4: メインアプリケーションの実装（repo_auditor.py）

以下の順序で実装を進めてください：

##### 4.1 インポートと定数定義

```python
#!/usr/bin/env python3
import tkinter as tk
from tkinter import ttk
import threading
import json
import os
from pathlib import Path
from datetime import datetime
import requests
from typing import Dict, List, Any
import time

# Python 3.11+ uses tomllib, 3.10 uses tomli
try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        tomllib = None  # Fallback to manual parsing
```

##### 4.2 RepoAuditorクラスの実装

**必須メソッド:**

1. `__init__(self, username: str, cache_dir: str)` - 初期化
2. `_get_headers(self)` - GitHub API用ヘッダー生成
3. `fetch_repositories(self, count: int)` - リポジトリ一覧取得
4. `fetch_repo_details(self, repo_name: str, check_items: List[str])` - ファイル存在確認

**キャッシュ実装の注意点:**
- `repos.json`: リポジトリ一覧を1時間キャッシュ
- `<repo_name>.json`: 個別リポジトリの結果を永続キャッシュ
- 新しいチェック項目が追加された場合、その項目のみ再取得

**GitHub API エンドポイント:**
- リポジトリ一覧: `GET /users/{username}/repos?sort=updated&per_page={count}`
- ファイル確認: `GET /repos/{username}/{repo}/contents/{filepath}`

**レート制限対策:**
- 環境変数`GITHUB_TOKEN`からPersonal Access Tokenを取得
- トークンなし: 60リクエスト/時間
- トークンあり: 5000リクエスト/時間

##### 4.3 ConfigWatcherクラスの実装

**必須メソッド:**

1. `__init__(self, config_path: str, callback)` - 初期化
2. `start(self)` - 監視開始
3. `stop(self)` - 監視停止
4. `_watch(self)` - ファイル変更監視ループ

**実装のポイント:**
- `os.stat().st_mtime`でファイル更新時刻を確認
- 1秒間隔でポーリング
- デーモンスレッドで実行（メインスレッドの終了を妨げない）

##### 4.4 RepoAuditorGUIクラスの実装

**必須メソッド:**

1. `__init__(self, root)` - GUI初期化
2. `_create_widgets(self)` - ウィジェット作成
3. `_load_config(self)` - TOML設定読み込み
4. `_update_tree_columns(self)` - Treeview列の動的更新
5. `_fetch_repos(self)` - リポジトリ取得（非同期）
6. `_update_display(self)` - 表示更新
7. `_update_status(self, message: str)` - ステータスバー更新

**GUI実装の注意点:**

- **Treeviewの列構成:**
  ```python
  columns = ["repo"] + check_items + ["updated"]
  self.tree["columns"] = columns
  self.tree["show"] = "headings"  # ツリーアイコンを非表示
  ```

- **非同期処理:**
  ```python
  def _fetch_repos(self):
      def fetch():
          # GitHub APIを呼び出す
          self.repos = self.auditor.fetch_repositories(20)
          # メインスレッドで表示更新
          self.root.after(0, self._update_display)
      
      threading.Thread(target=fetch, daemon=True).start()
  ```

- **カラーコーディング:**
  ```python
  self.tree.tag_configure("latest", background="#e3f2fd")     # 青色
  self.tree.tag_configure("missing", background="#ffebee")    # 赤色
  ```

##### 4.5 TOMLパーサーの実装（fallback用）

Python 3.10以下または`tomllib`が利用できない場合のフォールバック:

```python
def _parse_toml_simple(self, path: Path) -> Dict:
    """シンプルなTOMLパーサー（基本的な構文のみサポート）"""
    config = {"check_items": [], "display": {}}
    current_section = None
    
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            
            if line.startswith("[") and line.endswith("]"):
                current_section = line[1:-1]
                if current_section not in config:
                    config[current_section] = {}
            elif "=" in line:
                key, value = line.split("=", 1)
                key = key.strip()
                value = value.strip()
                
                # 値のパース
                if value == "true":
                    value = True
                elif value == "false":
                    value = False
                elif value.startswith("[") and value.endswith("]"):
                    # 配列のパース
                    items = value[1:-1].split(",")
                    value = [item.strip().strip('"').strip("'") 
                            for item in items if item.strip()]
                else:
                    value = value.strip('"').strip("'")
                
                if current_section:
                    config[current_section][key] = value
                else:
                    config[key] = value
    
    return config
```

#### Step 5: エントリーポイントの実装

```python
def main():
    root = tk.Tk()
    app = RepoAuditorGUI(root)
    root.mainloop()

if __name__ == "__main__":
    main()
```

### テスト戦略

#### 単体テスト

各クラスの主要メソッドをテスト:

```python
# test_repo_auditor.py
import unittest
from repo_auditor import RepoAuditor, ConfigWatcher

class TestRepoAuditor(unittest.TestCase):
    def setUp(self):
        self.auditor = RepoAuditor("testuser", ".cache_test")
    
    def test_fetch_repositories_with_cache(self):
        # キャッシュが正しく動作するかテスト
        repos1 = self.auditor.fetch_repositories(5)
        repos2 = self.auditor.fetch_repositories(5)
        self.assertEqual(repos1, repos2)
    
    def test_fetch_repo_details_incremental(self):
        # 差分取得が正しく動作するかテスト
        details1 = self.auditor.fetch_repo_details("test-repo", ["README.md"])
        details2 = self.auditor.fetch_repo_details("test-repo", 
            ["README.md", "LICENSE"])
        self.assertIn("LICENSE", details2)
```

#### 統合テスト

実際のGitHub APIを使用したテスト:

```bash
export GITHUB_TOKEN=your_test_token
python -m pytest tests/integration/
```

#### 手動テスト手順

1. `python repo_auditor.py`で起動
2. リポジトリ一覧が表示されることを確認
3. `audit_config.toml`に新しい項目を追加
4. 1-2秒待ち、自動リロードされることを確認
5. "Reload"ボタンをクリックし、再取得が動作することを確認

### エラーハンドリング

#### GitHub API エラー

```python
try:
    response = requests.get(url, headers=headers)
    response.raise_for_status()
except requests.exceptions.HTTPError as e:
    if e.response.status_code == 403:
        # Rate limit exceeded
        print("API rate limit exceeded. Please set GITHUB_TOKEN.")
    elif e.response.status_code == 404:
        # Repository not found
        print(f"Repository not found: {repo_name}")
    else:
        raise
```

#### ネットワークエラー

```python
try:
    response = requests.get(url, headers=headers, timeout=10)
except requests.exceptions.Timeout:
    print("Request timed out. Please check your network connection.")
except requests.exceptions.ConnectionError:
    print("Failed to connect to GitHub API.")
```

#### ファイルシステムエラー

```python
try:
    with open(cache_file, "w") as f:
        json.dump(data, f, indent=2)
except PermissionError:
    print(f"Permission denied: {cache_file}")
except OSError as e:
    print(f"Failed to write cache: {e}")
```

### パフォーマンス最適化

#### キャッシュヒット率の向上

```python
# リポジトリ一覧のキャッシュ有効期限を適切に設定
CACHE_EXPIRY_SECONDS = 3600  # 1時間

# mtimeチェックでキャッシュの鮮度を確認
cache_age = time.time() - cache_file.stat().st_mtime
if cache_age < CACHE_EXPIRY_SECONDS:
    return cached_data
```

#### API呼び出しの最小化

```python
# 新しいチェック項目のみ取得
items_to_fetch = [item for item in check_items if item not in cached_data]
if items_to_fetch:
    for item in items_to_fetch:
        # APIを呼び出し
        ...
    # キャッシュを更新
    cached_data.update(new_results)
```

#### GUI応答性の維持

```python
# 重い処理は別スレッドで実行
def _fetch_repos(self):
    def fetch():
        # GitHub APIを呼び出す
        repos = self.auditor.fetch_repositories(20)
        # メインスレッドで表示更新
        self.root.after(0, lambda: self._update_display(repos))
    
    threading.Thread(target=fetch, daemon=True).start()
```

### デバッグのヒント

#### ログ出力の追加

```python
import logging

logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

logger = logging.getLogger(__name__)

# 使用例
logger.debug(f"Fetching repositories for user: {self.username}")
logger.info(f"Cache hit for {repo_name}")
logger.warning(f"API rate limit approaching: {remaining} requests left")
```

#### キャッシュの検証

```bash
# キャッシュディレクトリの内容を確認
ls -lh .cache/

# 個別キャッシュファイルの内容を確認
cat .cache/repos.json | jq .
cat .cache/my-repo.json | jq .
```

#### GitHub API のレート制限確認

```python
response = requests.get(url, headers=headers)
remaining = response.headers.get("X-RateLimit-Remaining")
reset_time = response.headers.get("X-RateLimit-Reset")
print(f"Rate limit: {remaining} requests remaining")
print(f"Resets at: {datetime.fromtimestamp(int(reset_time))}")
```

### Coding Agentへの推奨プロンプト

以下は、Coding Agentに本プロジェクトを実装させる際の推奨プロンプト例です：

```
あなたは熟練したPythonエンジニアです。以下の仕様に基づいて、
GitHubリポジトリ監査ツール「cat-repo-auditor」をゼロから実装してください。

【要件】
1. Python 3.10以上で動作すること
2. Tkinterを使用したGUIアプリケーション
3. GitHub APIを使用してユーザーのリポジトリを取得
4. TOML形式の設定ファイルでチェック項目を定義
5. 設定ファイルのホットリロード機能
6. インテリジェントなキャッシュ機構

【実装手順】
1. プロジェクト構造を作成
2. requirements.txtを作成
3. audit_config.tomlのデフォルト設定を作成
4. RepoAuditorクラスを実装
5. ConfigWatcherクラスを実装
6. RepoAuditorGUIクラスを実装
7. エントリーポイントを実装

【参考】
詳細な実装ガイドは README.ja.md の「開発ガイド（Coding Agent向け）」セクションを参照してください。

【制約】
- PEP 8コーディング規約に準拠
- 型ヒントを適切に使用
- エラーハンドリングを適切に実装
- コメントは日本語で記述
```

## カスタマイズ例

### ユーザー名の変更

`repo_auditor.py`の以下の行を編集：

```python
self.username = "your_github_username"  # ここを変更
```

### チェック項目のプリセット

プロジェクトタイプ別の設定例：

**Python プロジェクト:**
```toml
check_items = [
    "README.md",
    "LICENSE",
    ".gitignore",
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "tests/",
    ".github/workflows/python-tests.yml",
]
```

**Node.js プロジェクト:**
```toml
check_items = [
    "README.md",
    "LICENSE",
    ".gitignore",
    "package.json",
    "package-lock.json",
    "tsconfig.json",
    "tests/",
    ".github/workflows/node-tests.yml",
]
```

**React プロジェクト:**
```toml
check_items = [
    "README.md",
    "LICENSE",
    "package.json",
    "public/",
    "src/",
    ".env.example",
    "Dockerfile",
    ".github/workflows/deploy.yml",
]
```

### 複数ユーザーの監査

設定ファイルにユーザー名を追加：

```toml
[users]
primary = "your_username"
secondary = "another_username"
```

コードで対応：

```python
users = self.config.get("users", {})
for key, username in users.items():
    auditor = RepoAuditor(username)
    # 監査処理
```

## トラブルシューティング

### 問題: GitHub API Rate Limit Exceeded

**症状:**
```
Error: API rate limit exceeded
```

**解決方法:**
1. GitHub Personal Access Tokenを作成
2. 環境変数に設定：
   ```bash
   export GITHUB_TOKEN=ghp_your_token_here
   ```
3. アプリケーションを再起動

### 問題: Tkinterが見つからない

**症状:**
```
ModuleNotFoundError: No module named '_tkinter'
```

**解決方法（Ubuntu/Debian）:**
```bash
sudo apt-get install python3-tk
```

**解決方法（macOS）:**
```bash
brew install python-tk
```

### 問題: 設定ファイルのホットリロードが動作しない

**解決方法:**
1. ファイルシステムのmtimeが正しく更新されているか確認
2. エディタの設定で「保存時に一時ファイルを作成しない」を確認
3. 手動で"Reload Config"ボタンをクリック

### 問題: 一部のリポジトリが表示されない

**原因:**
- Private リポジトリの可能性
- Personal Access Tokenに`repo`スコープが必要

**解決方法:**
トークンに以下のスコープを付与：
- `repo`（プライベートリポジトリへのアクセス）
- `read:org`（組織リポジトリへのアクセス）

### コードスタイル

- PEP 8に準拠
- 型ヒントを使用
- docstringをGoogleスタイルで記述

### テスト

新機能を追加する場合、対応するテストも追加してください：

```bash
python -m pytest tests/
```
