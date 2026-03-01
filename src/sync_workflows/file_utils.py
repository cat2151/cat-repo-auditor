import hashlib
import shutil
from pathlib import Path


def file_sha256(path: Path) -> str:
    """ファイルの SHA-256 ハッシュを返す（バイト列ベース）。"""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bytes_sha256(data: bytes) -> str:
    """バイト列の SHA-256 ハッシュを返す。"""
    return hashlib.sha256(data).hexdigest()


def copy_file(src: Path, dst: Path) -> None:
    """ファイルを上書きコピーする。親ディレクトリがなければ作成する。"""
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(str(src), str(dst))


def normalize_line_endings(data: bytes) -> bytes:
    """改行コードを LF に正規化したバイト列を返す。"""
    return data.replace(b"\r\n", b"\n").replace(b"\r", b"\n")


def count_line_endings(data: bytes) -> tuple[int, int, int]:
    """改行コードの内訳を (CRLF, LF, CR) で返す。"""
    crlf_count = data.count(b"\r\n")
    rest = data.replace(b"\r\n", b"")
    lf_count = rest.count(b"\n")
    cr_count = rest.count(b"\r")
    return crlf_count, lf_count, cr_count


def is_line_ending_only_diff(data_a: bytes, data_b: bytes) -> bool:
    """内容は同一で改行コードのみ異なる場合に True を返す。"""
    return data_a != data_b and normalize_line_endings(data_a) == normalize_line_endings(data_b)
