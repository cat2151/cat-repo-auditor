# ---------------------------------------------------------------------------
# status 値
# ---------------------------------------------------------------------------

STATUS_PULLABLE   = "pullable"    # 今すぐ pull 可能
STATUS_DIVERGED   = "diverged"    # behind かつ ahead（要注意）
STATUS_UP_TO_DATE = "up_to_date"  # 最新
STATUS_UNKNOWN    = "unknown"     # 判定不能（fetch 失敗・dirty で behind あり など）

# ---------------------------------------------------------------------------
# デフォルトファイルパス
# ---------------------------------------------------------------------------

DEFAULT_CONFIG = "config.toml"
DEFAULT_OUTPUT = "github_local_checker_result.json"

# ---------------------------------------------------------------------------
# 表示ラベル・カラー
# ---------------------------------------------------------------------------

STATUS_LABEL = {
    STATUS_PULLABLE  : "PULLABLE   ✓",
    STATUS_DIVERGED  : "DIVERGED   ⚠",
    STATUS_UP_TO_DATE: "UP-TO-DATE  ",
    STATUS_UNKNOWN   : "UNKNOWN    ?",
}

_COLOR = {
    STATUS_PULLABLE  : "\033[32m",   # 緑
    STATUS_DIVERGED  : "\033[33m",   # 黄
    STATUS_UP_TO_DATE: "\033[0m",    # デフォルト
    STATUS_UNKNOWN   : "\033[31m",   # 赤
}
_RESET = "\033[0m"
