"""
config.toml からユーザー設定を読み込む。
"""

import sys
from pathlib import Path

try:
    import tomllib          # Python 3.11+
except ImportError:
    try:
        import tomli as tomllib  # pip install tomli
    except ImportError:
        tomllib = None

from .colors import C

DEEPWIKI_PATTERNS = ["deepwiki.com", "deepwiki", "DeepWiki"]


def load_config(config_path: str = "config.toml") -> dict:
    """カレントディレクトリの config.toml を読み込む。

    必須キー: github_user
    例:
        github_user = "your-github-username"
    """
    p = Path(config_path)
    if not p.exists():
        print(f"{C.NG_RED}ERROR{C.RESET}: {config_path} が見つからない。", file=sys.stderr)
        print("  カレントディレクトリに以下の内容で作成してくれ:", file=sys.stderr)
        print('  github_user = "your-github-username"\n', file=sys.stderr)
        sys.exit(1)
    if tomllib is None:
        print(f"{C.NG_RED}ERROR{C.RESET}: TOML パーサーが見つからない。", file=sys.stderr)
        print("  Python 3.11+ を使うか `pip install tomli` を実行してくれ。", file=sys.stderr)
        sys.exit(1)
    with open(p, "rb") as f:
        cfg = tomllib.load(f)
    if "github_user" not in cfg:
        print(f"{C.NG_RED}ERROR{C.RESET}: config.toml に github_user が定義されていない。", file=sys.stderr)
        sys.exit(1)
    return cfg
