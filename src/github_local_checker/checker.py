from pathlib import Path

try:
    from .constants import (
        STATUS_DIVERGED,
        STATUS_PULLABLE,
        STATUS_UNKNOWN,
        STATUS_UP_TO_DATE,
    )
    from .git_utils import (
        fetch_remote,
        get_behind_ahead,
        get_current_branch,
        get_remote_url,
        is_dirty,
        is_git_repo,
        is_target_repo,
    )
except ImportError:
    from constants import (
        STATUS_DIVERGED,
        STATUS_PULLABLE,
        STATUS_UNKNOWN,
        STATUS_UP_TO_DATE,
    )
    from git_utils import (
        fetch_remote,
        get_behind_ahead,
        get_current_branch,
        get_remote_url,
        is_dirty,
        is_git_repo,
        is_target_repo,
    )


def classify(dirty: bool, behind: int, ahead: int) -> str:
    """
    pullable  : not dirty, behind > 0, ahead == 0
    diverged  : behind > 0, ahead > 0  （dirty の有無に関わらず diverged を優先表示）
    up_to_date: behind == 0
    unknown   : 上記以外（取得不能・dirty で behind あり など）
    """
    if behind < 0 or ahead < 0:
        return STATUS_UNKNOWN
    if behind > 0 and ahead > 0:
        return STATUS_DIVERGED
    if behind == 0:
        return STATUS_UP_TO_DATE
    # behind > 0, ahead == 0 のケース
    if not dirty:
        return STATUS_PULLABLE
    # dirty かつ behind > 0 → pull したいが今は不可
    return STATUS_UNKNOWN


def check_repo(path: str, github_username: str) -> dict:
    """
    リポジトリを解析して結果 dict を返す。

    {
        "path"       : str,
        "name"       : str,
        "is_target"  : bool,
        "remote_url" : str | null,
        "branch"     : str | null,
        "dirty"      : bool | null,
        "behind"     : int | null,
        "ahead"      : int | null,
        "status"     : "pullable" | "diverged" | "up_to_date" | "unknown" | null,
        "error"      : str | null,
    }
    """
    result: dict = {
        "path"      : path,
        "name"      : Path(path).name,
        "is_target" : False,
        "remote_url": None,
        "branch"    : None,
        "dirty"     : None,
        "behind"    : None,
        "ahead"     : None,
        "status"    : None,
        "error"     : None,
    }
    errors: list[str] = []

    # --- git リポジトリか ---
    if not is_git_repo(path):
        result["error"] = "git リポジトリではない"
        return result

    # --- remote URL 取得 ---
    remote_url = get_remote_url(path)
    result["remote_url"] = remote_url
    if remote_url is None:
        result["error"] = "origin が設定されていない"
        return result

    # --- 対象ユーザーか ---
    if not is_target_repo(remote_url, github_username):
        return result  # is_target = False のまま返す

    result["is_target"] = True

    # --- ブランチ取得 ---
    branch = get_current_branch(path)
    result["branch"] = branch
    if branch is None or branch == "HEAD":
        result["error"] = "detached HEAD 状態か、ブランチ名取得失敗"
        result["status"] = STATUS_UNKNOWN
        return result

    # --- dirty チェック ---
    dirty = is_dirty(path)
    result["dirty"] = dirty

    # --- fetch（常に実行） ---
    fetch_ok, fetch_err = fetch_remote(path)
    if not fetch_ok:
        errors.append(fetch_err or "git fetch 失敗")

    # --- behind / ahead ---
    behind, ahead = get_behind_ahead(path, branch)
    if behind >= 0:
        result["behind"] = behind
        result["ahead"]  = ahead
    else:
        errors.append("tracking ブランチが見つからない（origin に対応ブランチがないかもしれない）")

    # --- 3分類 ---
    result["status"] = classify(dirty, behind, ahead)

    if errors:
        result["error"] = " / ".join(errors)

    return result
