from collections import Counter
from pathlib import Path

try:
    from .file_utils import file_sha256
except ImportError:
    from file_utils import file_sha256


def check_one(sync_filepath: Path, target_repos: list[Path]) -> bool:
    """
    1ファイルについてハッシュ比較を行い、全一致なら True を返す。
    欠落リポジトリも不一致として扱う。
    """
    print(f"=== {sync_filepath.as_posix()} ===")

    hashes: dict[Path, str] = {}
    for d in target_repos:
        fp = d / sync_filepath
        hashes[d] = file_sha256(fp) if fp.exists() else "(欠落)"

    print("[HASH]")
    for d, digest in hashes.items():
        print(f"  {d.name:<30}  {digest}")

    existing = [h for h in hashes.values() if h != "(欠落)"]
    if not existing:
        print("[WARN] すべてのリポジトリでファイルが欠落している。")
        print()
        return False

    majority_hash = Counter(existing).most_common(1)[0][0]
    all_ok = all(h == majority_hash for h in hashes.values())

    if all_ok:
        print("[OK] すべて一致。")
        print()
        return True
    else:
        print("[WARN] 不一致あり。")
        print(f"  基準ハッシュ (最多一致): {majority_hash}")
        for d, digest in sorted(hashes.items()):
            if digest != majority_hash:
                print(f"  対象: {d.name:<30}  {digest}")
        print()
        return False
