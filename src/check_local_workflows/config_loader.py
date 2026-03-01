import sys
import tomllib

try:
    from .constants import CONFIG_FILE
except ImportError:
    from constants import CONFIG_FILE


def load_sync_filepaths() -> list:
    """config.toml から sync_filepaths を読み込む。"""
    if not CONFIG_FILE.exists():
        print(f"[ERROR] config.toml が見つからない: {CONFIG_FILE}")
        sys.exit(1)
    with open(CONFIG_FILE, "rb") as f:
        config = tomllib.load(f)
    paths = config.get("sync", {}).get("sync_filepaths", [])
    if not paths:
        print("[ERROR] config.toml に sync_filepaths が設定されていない。")
        sys.exit(1)
    from pathlib import Path
    return [Path(p) for p in paths]
