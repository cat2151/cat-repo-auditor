# cat-repo-auditor 実装ガイド - Coding Agent用完全版

このドキュメントは、GitHub Copilot、Cursor、Windsurf、Claude Code などのCoding Agentが、
`cat-repo-auditor`を**ゼロから完璧に実装する**ための詳細ガイドです。

## 目次

1. [プロジェクト概要](#プロジェクト概要)
2. [技術スタック](#技術スタック)
3. [完全実装手順](#完全実装手順)
4. [各クラスの詳細実装](#各クラスの詳細実装)
5. [エッジケースとエラーハンドリング](#エッジケースとエラーハンドリング)
6. [テストシナリオ](#テストシナリオ)
7. [デバッグチェックリスト](#デバッグチェックリスト)

---

## プロジェクト概要

### 目的

GitHubユーザーの複数リポジトリを監査し、標準化されたファイル構成の遵守状況を可視化する。

### コア機能

1. **リポジトリ取得**: GitHub APIで直近20リポジトリを取得
2. **ファイルチェック**: 各リポジトリの指定ファイルの存在を確認
3. **差分可視化**: 最新リポジトリとの差分を強調表示
4. **設定駆動**: TOMLファイルでチェック項目を定義
5. **ホットリロード**: 設定変更を自動検知
6. **キャッシュ**: APIコールを最小化

### 非機能要件

- Python 3.10以上
- クロスプラットフォーム（Windows/Mac/Linux）
- 外部依存は最小限（requestsのみ）
- GUI応答性の維持（非同期処理）
- APIレート制限への対応

---

## 技術スタック

### 必須ライブラリ

```python
# 標準ライブラリ
import tkinter as tk                 # GUI
from tkinter import ttk              # モダンなウィジェット
import threading                     # 非同期処理
import json                          # キャッシュ
import os                            # 環境変数
from pathlib import Path             # ファイルパス
from datetime import datetime        # 日時処理
from typing import Dict, List, Any   # 型ヒント
import time                          # 時間計測

# 外部ライブラリ
import requests                      # HTTP通信

# TOML解析（Python 3.11+）
try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib
    except ImportError:
        tomllib = None  # 手動パーサーにフォールバック
```

### GitHub API エンドポイント

| 用途 | エンドポイント | メソッド |
|------|--------------|---------|
| リポジトリ一覧 | `/users/{username}/repos` | GET |
| ファイル確認 | `/repos/{username}/{repo}/contents/{path}` | GET |

---

## 完全実装手順

### Phase 1: プロジェクト初期化（5分）

```bash
# ディレクトリ作成
mkdir cat-repo-auditor
cd cat-repo-auditor

# ファイル作成
touch repo_auditor.py
touch audit_config.toml
touch requirements.txt
touch README.md
touch README.ja.md
touch LICENSE

# Git初期化（オプション）
git init
echo ".cache/" >> .gitignore
echo "__pycache__/" >> .gitignore
echo "*.pyc" >> .gitignore
```

### Phase 2: 依存関係定義（2分）

**requirements.txt:**

```
requests>=2.31.0
# Note: tomllib is built into Python 3.11+
# For Python 3.10, install: tomli>=2.0.0
```

### Phase 3: デフォルト設定作成（3分）

**audit_config.toml:**

```toml
# Repository Auditor Configuration
# このファイルを編集すると自動的にリロードされる

# チェックする項目（ファイルパス）
check_items = [
    "README.md",
    "LICENSE",
    ".gitignore",
    "CONTRIBUTING.md",
    ".github/workflows/ci.yml",
]

# 表示設定
[display]
show_repo_name = true
show_updated_at = true
highlight_missing = true  # 最新リポジトリにあるが他にないものを強調
```

### Phase 4: メインアプリケーション実装（60分）

以下、`repo_auditor.py`の完全実装を段階的に説明します。

---

## 各クラスの詳細実装

### 1. RepoAuditor クラス

#### 目的

GitHub APIとの通信、キャッシュ管理を担当。

#### 完全実装コード

```python
class RepoAuditor:
    """GitHubリポジトリの監査ロジック"""
    
    def __init__(self, username: str, cache_dir: str = ".cache"):
        """
        初期化
        
        Args:
            username: GitHubユーザー名
            cache_dir: キャッシュディレクトリのパス
        """
        self.username = username
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(exist_ok=True)
        self.token = os.getenv("GITHUB_TOKEN", "")
        
        # APIエンドポイント
        self.api_base = "https://api.github.com"
    
    def _get_headers(self) -> Dict[str, str]:
        """
        GitHub API用のHTTPヘッダーを生成
        
        Returns:
            ヘッダー辞書
        """
        headers = {
            "Accept": "application/vnd.github.v3+json",
            "User-Agent": "cat-repo-auditor/1.0"
        }
        
        # トークンがあれば認証ヘッダーを追加
        if self.token:
            headers["Authorization"] = f"token {self.token}"
        
        return headers
    
    def fetch_repositories(self, count: int = 20) -> List[Dict]:
        """
        直近のリポジトリを取得（キャッシュ対応）
        
        Args:
            count: 取得するリポジトリ数
        
        Returns:
            リポジトリ情報のリスト
        
        Raises:
            requests.exceptions.HTTPError: API呼び出し失敗
        """
        cache_file = self.cache_dir / "repos.json"
        
        # キャッシュチェック（1時間以内なら再利用）
        if cache_file.exists():
            cache_age = time.time() - cache_file.stat().st_mtime
            if cache_age < 3600:  # 1時間 = 3600秒
                with open(cache_file) as f:
                    cached = json.load(f)
                    if len(cached) >= count:
                        print(f"Using cached repository list ({len(cached)} repos)")
                        return cached[:count]
        
        # GitHub APIから取得
        print(f"Fetching repositories for {self.username}...")
        url = f"{self.api_base}/users/{self.username}/repos"
        params = {
            "sort": "updated",       # 更新日時順
            "per_page": count,       # 取得数
            "type": "all"            # すべてのリポジトリ
        }
        
        try:
            response = requests.get(
                url, 
                headers=self._get_headers(), 
                params=params,
                timeout=10
            )
            response.raise_for_status()
            
            repos = response.json()
            
            # レート制限情報を表示
            remaining = response.headers.get("X-RateLimit-Remaining")
            if remaining:
                print(f"API rate limit: {remaining} requests remaining")
            
            # キャッシュに保存
            with open(cache_file, "w") as f:
                json.dump(repos, f, indent=2)
            
            print(f"Fetched {len(repos)} repositories")
            return repos
            
        except requests.exceptions.Timeout:
            print("Error: Request timed out. Please check your network.")
            raise
        except requests.exceptions.ConnectionError:
            print("Error: Failed to connect to GitHub API.")
            raise
        except requests.exceptions.HTTPError as e:
            if e.response.status_code == 403:
                print("Error: API rate limit exceeded. Please set GITHUB_TOKEN.")
            elif e.response.status_code == 404:
                print(f"Error: User '{self.username}' not found.")
            raise
    
    def fetch_repo_details(
        self, 
        repo_name: str, 
        check_items: List[str]
    ) -> Dict[str, bool]:
        """
        リポジトリの詳細をチェック（キャッシュ対応）
        
        Args:
            repo_name: リポジトリ名
            check_items: チェックするファイル/ディレクトリのリスト
        
        Returns:
            {ファイルパス: 存在するか} の辞書
        """
        cache_file = self.cache_dir / f"{repo_name}.json"
        
        # キャッシュから読み込み
        cached_data = {}
        if cache_file.exists():
            with open(cache_file) as f:
                cached_data = json.load(f)
        
        # 必要な項目のみ再取得
        results = {}
        items_to_fetch = []
        
        for item in check_items:
            if item in cached_data:
                results[item] = cached_data[item]
            else:
                items_to_fetch.append(item)
        
        # 新規項目があれば取得
        if items_to_fetch:
            print(f"Checking {len(items_to_fetch)} new items for {repo_name}...")
            
            for item in items_to_fetch:
                url = f"{self.api_base}/repos/{self.username}/{repo_name}/contents/{item}"
                
                try:
                    response = requests.get(
                        url, 
                        headers=self._get_headers(),
                        timeout=5
                    )
                    results[item] = response.status_code == 200
                    
                except requests.exceptions.RequestException:
                    # ネットワークエラーの場合、存在しないとみなす
                    results[item] = False
                
                # APIレート制限対策（軽微な遅延）
                time.sleep(0.1)
            
            # キャッシュを更新
            cached_data.update(results)
            with open(cache_file, "w") as f:
                json.dump(cached_data, f, indent=2)
        
        return results
```

#### 実装のポイント

**キャッシュ戦略:**
- `repos.json`: 時限キャッシュ（1時間）
- `{repo_name}.json`: 永続キャッシュ（設定変更時に差分更新）

**エラーハンドリング:**
- タイムアウト: 10秒
- レート制限: ヘッダーから残数を表示
- 404エラー: ユーザー名が間違っている可能性を示唆

**パフォーマンス:**
- 新規項目のみ取得（差分更新）
- API呼び出し間に0.1秒の遅延（レート制限対策）

---

### 2. ConfigWatcher クラス

#### 目的

TOMLファイルの変更を監視し、ホットリロードを実現。

#### 完全実装コード

```python
class ConfigWatcher:
    """TOMLファイルのホットリロード監視"""
    
    def __init__(self, config_path: str, callback):
        """
        初期化
        
        Args:
            config_path: 監視する設定ファイルのパス
            callback: ファイル変更時に呼び出す関数
        """
        self.config_path = Path(config_path)
        self.callback = callback
        self.last_mtime = 0
        self.running = False
        self.thread = None
    
    def start(self):
        """監視開始"""
        if not self.running:
            self.running = True
            self.thread = threading.Thread(target=self._watch, daemon=True)
            self.thread.start()
            print(f"Config watcher started for {self.config_path}")
    
    def stop(self):
        """監視停止"""
        self.running = False
        if self.thread:
            self.thread.join(timeout=2)
        print("Config watcher stopped")
    
    def _watch(self):
        """ファイル変更を監視（バックグラウンドスレッド）"""
        while self.running:
            try:
                if self.config_path.exists():
                    current_mtime = self.config_path.stat().st_mtime
                    
                    # 初回またはファイルが変更された場合
                    if self.last_mtime == 0:
                        self.last_mtime = current_mtime
                    elif current_mtime != self.last_mtime:
                        self.last_mtime = current_mtime
                        print(f"Config file changed: {self.config_path}")
                        # コールバックを実行
                        self.callback()
                
            except Exception as e:
                print(f"Error in config watcher: {e}")
            
            # 1秒待機
            time.sleep(1)
```

#### 実装のポイント

**スレッド管理:**
- `daemon=True`: メインスレッド終了時に自動終了
- `threading.Thread`: GUIをブロックしない

**ファイル監視:**
- `os.stat().st_mtime`: 最終更新時刻を取得
- 1秒間隔でポーリング（低負荷）

**初回起動時の考慮:**
- `last_mtime == 0`: 初回は変更とみなさない

---

### 3. RepoAuditorGUI クラス

#### 目的

Tkinterベースのグラフィカルユーザーインターフェース。

#### 完全実装コード（長いため分割）

##### 3.1 初期化

```python
class RepoAuditorGUI:
    """Tkinter GUI"""
    
    def __init__(self, root):
        """
        GUI初期化
        
        Args:
            root: Tkルートウィンドウ
        """
        self.root = root
        self.root.title("Repository Auditor - cat-repo-auditor")
        self.root.geometry("1200x800")
        
        # デフォルトユーザー名（後で設定から読み込む）
        self.username = "cat2151"
        
        self.auditor = RepoAuditor(self.username)
        self.config_path = Path("audit_config.toml")
        self.config = {}
        self.repos = []
        
        # ウィジェット作成
        self._create_widgets()
        
        # 設定読み込み
        self._load_config()
        
        # 設定ファイル監視
        self.watcher = ConfigWatcher(self.config_path, self._on_config_changed)
        self.watcher.start()
        
        # 初回リポジトリ取得
        self._fetch_repos()
        
        # ウィンドウクローズ時の処理
        self.root.protocol("WM_DELETE_WINDOW", self._on_closing)
```

##### 3.2 ウィジェット作成

```python
    def _create_widgets(self):
        """ウィジェット作成"""
        
        # === トップバー ===
        top_frame = ttk.Frame(self.root, padding="10")
        top_frame.pack(fill=tk.X)
        
        # ユーザー名表示
        ttk.Label(
            top_frame, 
            text=f"GitHub: {self.username}", 
            font=("Arial", 12, "bold")
        ).pack(side=tk.LEFT)
        
        # ボタン
        ttk.Button(
            top_frame, 
            text="Reload Repos", 
            command=self._fetch_repos
        ).pack(side=tk.RIGHT, padx=5)
        
        ttk.Button(
            top_frame, 
            text="Reload Config", 
            command=self._load_config
        ).pack(side=tk.RIGHT)
        
        # === メインエリア ===
        main_frame = ttk.Frame(self.root, padding="10")
        main_frame.pack(fill=tk.BOTH, expand=True)
        
        # Treeview用フレーム
        tree_frame = ttk.Frame(main_frame)
        tree_frame.pack(fill=tk.BOTH, expand=True)
        
        # スクロールバー
        scrollbar = ttk.Scrollbar(tree_frame, orient=tk.VERTICAL)
        scrollbar.pack(side=tk.RIGHT, fill=tk.Y)
        
        # Treeview（列は動的に変更される）
        self.tree = ttk.Treeview(
            tree_frame, 
            yscrollcommand=scrollbar.set,
            selectmode="browse"
        )
        self.tree.pack(fill=tk.BOTH, expand=True)
        scrollbar.config(command=self.tree.yview)
        
        # === ステータスバー ===
        status_frame = ttk.Frame(self.root, padding="5")
        status_frame.pack(fill=tk.X, side=tk.BOTTOM)
        
        self.status_label = ttk.Label(
            status_frame, 
            text="Ready", 
            relief=tk.SUNKEN,
            anchor=tk.W
        )
        self.status_label.pack(fill=tk.X)
```

##### 3.3 設定読み込み

```python
    def _parse_toml_simple(self, path: Path) -> Dict:
        """
        シンプルなTOMLパーサー（fallback用）
        
        Args:
            path: TOMLファイルのパス
        
        Returns:
            パース結果の辞書
        """
        config = {"check_items": [], "display": {}}
        current_section = None
        
        with open(path) as f:
            for line in f:
                line = line.strip()
                
                # コメントと空行をスキップ
                if not line or line.startswith("#"):
                    continue
                
                # セクションヘッダー
                if line.startswith("[") and line.endswith("]"):
                    current_section = line[1:-1]
                    if current_section not in config:
                        config[current_section] = {}
                
                # キー=値
                elif "=" in line:
                    key, value = line.split("=", 1)
                    key = key.strip()
                    value = value.strip()
                    
                    # 値の型変換
                    if value == "true":
                        value = True
                    elif value == "false":
                        value = False
                    elif value.startswith("[") and value.endswith("]"):
                        # 配列のパース
                        items = value[1:-1].split(",")
                        value = [
                            item.strip().strip('"').strip("'") 
                            for item in items 
                            if item.strip()
                        ]
                    else:
                        # 文字列（クォート除去）
                        value = value.strip('"').strip("'")
                    
                    # 辞書に格納
                    if current_section:
                        config[current_section][key] = value
                    else:
                        config[key] = value
        
        return config
    
    def _load_config(self):
        """TOMLから設定を読み込み"""
        
        # 設定ファイルが存在しない場合、デフォルトを作成
        if not self.config_path.exists():
            default_config_toml = '''# Repository Auditor Configuration
check_items = [
    "README.md",
    "LICENSE",
    ".gitignore",
]

[display]
show_repo_name = true
show_updated_at = true
highlight_missing = true
'''
            with open(self.config_path, "w") as f:
                f.write(default_config_toml)
            print(f"Created default config: {self.config_path}")
        
        # TOMLをパース
        if tomllib:
            # Python 3.11+
            with open(self.config_path, "rb") as f:
                self.config = tomllib.load(f)
        else:
            # Fallback
            self.config = self._parse_toml_simple(self.config_path)
        
        self._update_status(
            f"Config loaded: {len(self.config.get('check_items', []))} items"
        )
        self._update_tree_columns()
```

##### 3.4 Treeview列の更新

```python
    def _update_tree_columns(self):
        """Treeviewの列を動的に更新"""
        check_items = self.config.get("check_items", [])
        
        # 列の構成
        columns = ["repo"] + check_items
        if self.config.get("display", {}).get("show_updated_at", True):
            columns.append("updated")
        
        # Treeviewに設定
        self.tree["columns"] = columns
        self.tree["show"] = "headings"  # ツリーアイコンを非表示
        
        # 各列のヘッダーと幅を設定
        for col in columns:
            if col == "repo":
                self.tree.heading(col, text="Repository")
                self.tree.column(col, width=200, anchor=tk.W)
            
            elif col == "updated":
                self.tree.heading(col, text="Updated")
                self.tree.column(col, width=100, anchor=tk.CENTER)
            
            else:
                # ファイル名を短縮表示（パスの最後の部分のみ）
                display_name = col.split("/")[-1]
                if len(display_name) > 15:
                    display_name = display_name[:12] + "..."
                
                self.tree.heading(col, text=display_name)
                self.tree.column(col, width=80, anchor=tk.CENTER)
```

##### 3.5 リポジトリ取得（非同期）

```python
    def _fetch_repos(self):
        """リポジトリを取得（非同期）"""
        self._update_status("Fetching repositories...")
        
        def fetch():
            try:
                # GitHub APIから取得
                self.repos = self.auditor.fetch_repositories(20)
                
                # メインスレッドで表示更新
                self.root.after(0, self._update_display)
                
            except Exception as e:
                # エラーをメインスレッドで表示
                error_msg = f"Error: {str(e)}"
                self.root.after(0, lambda: self._update_status(error_msg))
        
        # バックグラウンドスレッドで実行
        threading.Thread(target=fetch, daemon=True).start()
```

##### 3.6 表示更新

```python
    def _update_display(self):
        """リポジトリ情報を表示"""
        
        # 既存の行をクリア
        for item in self.tree.get_children():
            self.tree.delete(item)
        
        if not self.repos:
            self._update_status("No repositories found")
            return
        
        check_items = self.config.get("check_items", [])
        highlight_missing = self.config.get("display", {}).get("highlight_missing", True)
        
        # 最新リポジトリ（基準）の詳細を取得
        latest_repo = self.repos[0]
        latest_details = self.auditor.fetch_repo_details(
            latest_repo["name"], 
            check_items
        )
        
        # 各リポジトリを表示
        for idx, repo in enumerate(self.repos):
            # 詳細を取得
            details = self.auditor.fetch_repo_details(repo["name"], check_items)
            
            # 表示する値のリスト
            values = [repo["name"]]
            tags = []
            
            # チェック項目の結果
            for item in check_items:
                has_item = details.get(item, False)
                latest_has_item = latest_details.get(item, False)
                
                # ✓ or ✗
                values.append("✓" if has_item else "✗")
                
                # 最新にはあるが、このリポジトリにはない場合
                if not has_item and latest_has_item and highlight_missing:
                    tags.append("missing")
            
            # 更新日時
            if self.config.get("display", {}).get("show_updated_at", True):
                updated = datetime.fromisoformat(
                    repo["updated_at"].replace("Z", "+00:00")
                )
                values.append(updated.strftime("%Y-%m-%d"))
            
            # 最新リポジトリは強調
            if idx == 0:
                tags.append("latest")
            
            # Treeviewに挿入
            self.tree.insert("", tk.END, values=values, tags=tags)
        
        # タグのスタイル設定
        self.tree.tag_configure("latest", background="#e3f2fd")   # 青色
        self.tree.tag_configure("missing", background="#ffebee")  # 赤色
        
        self._update_status(f"Loaded {len(self.repos)} repositories")
```

##### 3.7 その他のメソッド

```python
    def _on_config_changed(self):
        """設定ファイル変更時のコールバック"""
        # メインスレッドで実行
        self.root.after(0, self._load_config)
        self.root.after(100, self._update_display)
    
    def _update_status(self, message: str):
        """ステータスバーを更新"""
        timestamp = datetime.now().strftime('%H:%M:%S')
        self.status_label.config(text=f"{message} [{timestamp}]")
    
    def _on_closing(self):
        """ウィンドウクローズ時の処理"""
        print("Closing application...")
        self.watcher.stop()
        self.root.destroy()
```

---

### 4. エントリーポイント

```python
def main():
    """アプリケーションのエントリーポイント"""
    root = tk.Tk()
    app = RepoAuditorGUI(root)
    
    try:
        root.mainloop()
    except KeyboardInterrupt:
        print("\nApplication interrupted by user")
        app.watcher.stop()

if __name__ == "__main__":
    main()
```

---

## エッジケースとエラーハンドリング

### 1. ネットワークエラー

```python
# タイムアウト
try:
    response = requests.get(url, timeout=10)
except requests.exceptions.Timeout:
    print("Request timed out")
    return []

# 接続エラー
except requests.exceptions.ConnectionError:
    print("No internet connection")
    return []
```

### 2. GitHub API エラー

```python
try:
    response.raise_for_status()
except requests.exceptions.HTTPError as e:
    if e.response.status_code == 403:
        # Rate limit
        reset_time = e.response.headers.get("X-RateLimit-Reset")
        print(f"Rate limit exceeded. Resets at {reset_time}")
    
    elif e.response.status_code == 404:
        # User not found
        print(f"User '{username}' not found")
    
    elif e.response.status_code == 401:
        # Invalid token
        print("Invalid GitHub token")
```

### 3. ファイルシステムエラー

```python
try:
    with open(cache_file, "w") as f:
        json.dump(data, f)
except PermissionError:
    print(f"Permission denied: {cache_file}")
except OSError as e:
    print(f"Failed to write: {e}")
```

### 4. TOML解析エラー

```python
try:
    if tomllib:
        with open(config_path, "rb") as f:
            config = tomllib.load(f)
except Exception as e:
    print(f"Failed to parse TOML: {e}")
    # デフォルト設定を使用
    config = {
        "check_items": ["README.md"],
        "display": {"highlight_missing": True}
    }
```

---

## テストシナリオ

### 単体テスト

```python
import unittest

class TestRepoAuditor(unittest.TestCase):
    def setUp(self):
        self.auditor = RepoAuditor("testuser", ".cache_test")
    
    def test_cache_works(self):
        """キャッシュが動作するか"""
        repos1 = self.auditor.fetch_repositories(5)
        repos2 = self.auditor.fetch_repositories(5)
        self.assertEqual(repos1, repos2)
    
    def test_incremental_fetch(self):
        """差分取得が動作するか"""
        details1 = self.auditor.fetch_repo_details("repo", ["README.md"])
        details2 = self.auditor.fetch_repo_details("repo", 
            ["README.md", "LICENSE"])
        
        # 2回目は差分のみ取得
        self.assertIn("README.md", details2)
        self.assertIn("LICENSE", details2)

if __name__ == "__main__":
    unittest.main()
```

### 統合テスト

```bash
# 実際のGitHub APIを使用
export GITHUB_TOKEN=your_token
python -m pytest tests/
```

### 手動テスト手順

1. ✅ 起動してリポジトリが表示される
2. ✅ `audit_config.toml`に項目を追加 → 自動リロード
3. ✅ "Reload Repos"ボタン → 再取得
4. ✅ 最新リポジトリが青色で表示
5. ✅ 欠落項目が赤色で表示

---

## デバッグチェックリスト

### 起動時

- [ ] Python 3.10以上か確認
- [ ] requestsがインストールされているか
- [ ] audit_config.tomlが存在するか（なければ自動生成）
- [ ] .cache/ディレクトリが作成されるか

### リポジトリ取得

- [ ] GitHub APIへの接続が成功するか
- [ ] レート制限の残数が表示されるか
- [ ] キャッシュファイルが作成されるか
- [ ] タイムアウトが適切に処理されるか

### ホットリロード

- [ ] ConfigWatcherが起動するか
- [ ] TOMLファイル保存後1-2秒で反映されるか
- [ ] 列が動的に変更されるか

### GUI

- [ ] Treeviewが正しく表示されるか
- [ ] スクロールが動作するか
- [ ] 色分けが適用されるか
- [ ] ステータスバーが更新されるか

---

## パフォーマンス最適化

### キャッシュヒット率

```python
# リポジトリ一覧: 1時間
CACHE_EXPIRY = 3600

# ファイルチェック: 永続
# 新規項目のみ取得
```

### API呼び出し削減

```python
# 差分更新
items_to_fetch = [item for item in check_items if item not in cached]

# バッチ処理（将来の拡張）
# GitHub GraphQL APIを使用して1回のリクエストで複数ファイルをチェック
```

### GUI応答性

```python
# 重い処理は別スレッド
threading.Thread(target=fetch, daemon=True).start()

# メインスレッドで表示更新
self.root.after(0, self._update_display)
```

---

## Coding Agentへの最終メッセージ

このガイドに従えば、`cat-repo-auditor`を**完璧に実装できる**。

### 実装の優先順位

1. **Phase 1-3**: プロジェクト初期化（10分）
2. **RepoAuditor**: コア機能（30分）
3. **ConfigWatcher**: ホットリロード（10分）
4. **RepoAuditorGUI**: GUI（30分）
5. **テスト**: 動作確認（20分）

### 実装時の注意点

- PEP 8に準拠
- 型ヒントを使用
- エラーハンドリングを適切に
- コメントは日本語

### デバッグのコツ

- `print()`でログを出力
- キャッシュを削除して再テスト（`rm -rf .cache/`）
- GitHub APIレート制限に注意

---

**これで完璧だ。おまえならできる。**
