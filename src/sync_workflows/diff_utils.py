import subprocess
import sys
from pathlib import Path

try:
    from .file_utils import count_line_endings, is_line_ending_only_diff
except ImportError:
    from file_utils import count_line_endings, is_line_ending_only_diff


def show_difft(
    path_a: Path,
    path_b: Path,
    label_a: str = "left",
    label_b: str = "right",
) -> None:
    """difft を使って2ファイルの差分を表示する。difft がなければ diff にフォールバック。"""
    try:
        data_a = path_a.read_bytes()
        data_b = path_b.read_bytes()
        if is_line_ending_only_diff(data_a, data_b):
            a_crlf, a_lf, a_cr = count_line_endings(data_a)
            b_crlf, b_lf, b_cr = count_line_endings(data_b)
            print("  [INFO] 改行コードのみが差分（内容差分なし）")
            print(f"         {label_a}: CRLF={a_crlf}, LF={a_lf}, CR={a_cr}")
            print(f"         {label_b}: CRLF={b_crlf}, LF={b_lf}, CR={b_cr}")
            return
    except OSError:
        pass

    try:
        completed = subprocess.run(
            ["difft", "--color", "always", str(path_a), str(path_b)],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=10,
            check=False,
        )
        if completed.stdout:
            print(completed.stdout, end="")
        if completed.stderr:
            print(completed.stderr, file=sys.stderr, end="")

        if completed.returncode == 0:
            if "No changes." in completed.stdout:
                print("  [INFO] 構文差異なし。text比較で再比較する (CRLF 等の差異を検出)：")
                completed2 = subprocess.run(
                    [
                        "difft",
                        "--color",
                        "always",
                        "--strip-cr",
                        "off",
                        "--override",
                        "*:text",
                        str(path_a),
                        str(path_b),
                    ],
                    timeout=10,
                    check=False,
                )
                if completed2.returncode == 0:
                    return
            else:
                return
    except FileNotFoundError:
        pass
    except subprocess.TimeoutExpired:
        print("  [INFO] difft がタイムアウトしたため、内容比較をスキップする。")
        return

    try:
        completed = subprocess.run(
            ["diff", "-u", str(path_a), str(path_b)], check=False
        )
        if completed.returncode in (0, 1):
            return
    except FileNotFoundError:
        pass

    print("  [INFO] difft/diff コマンドが見つからない、または実行に失敗したため、内容比較をスキップする。")
