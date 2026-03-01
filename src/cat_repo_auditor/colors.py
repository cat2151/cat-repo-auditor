"""
ANSI カラー定数と文字列フォーマットヘルパー。
"""

# Monokai 256色パレット
class C:
    RESET  = "\033[0m"
    BOLD   = "\033[1m"
    OK_GRN = "\033[38;5;148m"   # #A6E22E 黄緑    : OK
    NG_RED = "\033[38;5;197m"   # #F92672 赤ピンク : NG
    TITLE  = "\033[38;5;81m"    # #66D9EF 水色    : セクションタイトル
    ORANGE = "\033[38;5;208m"   # #FD971F オレンジ : ヘッダ強調
    REPO   = "\033[38;5;228m"   # #E6DB74 黄      : リポジトリ名
    PURPLE = "\033[38;5;141m"   # #AE81FF 紫      : カウント
    DIM    = "\033[38;5;242m"   # #75715E グレー   : dim補足
    FG     = "\033[38;5;231m"   # #F8F8F2 白前景  : 通常テキスト

def ok(text: str) -> str:   return f"{C.OK_GRN}{text}{C.RESET}"
def ng(text: str) -> str:   return f"{C.NG_RED}{text}{C.RESET}"
def head(text: str) -> str: return f"{C.TITLE}{C.BOLD}{text}{C.RESET}"
def dim(text: str) -> str:  return f"{C.DIM}{text}{C.RESET}"
def repo(text: str) -> str: return f"{C.REPO}{text}{C.RESET}"
def hl(text: str) -> str:   return f"{C.ORANGE}{C.BOLD}{text}{C.RESET}"
