import hashlib
from pathlib import Path


def file_sha256(path: Path) -> str:
    """ファイルの SHA-256 ハッシュを返す（バイト列ベース）。"""
    return hashlib.sha256(path.read_bytes()).hexdigest()
