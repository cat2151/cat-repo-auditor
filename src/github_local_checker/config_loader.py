import sys
from pathlib import Path

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


def load_config(config_path: str) -> dict:
    path = Path(config_path)
    if not path.exists():
        print(f"ERROR: 設定ファイルが見つからない: {config_path}")
        sys.exit(1)
    with open(path, "rb") as f:
        return tomllib.load(f)
