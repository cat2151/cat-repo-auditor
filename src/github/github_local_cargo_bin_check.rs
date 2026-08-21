//! installed binary 自身の `check` サブコマンドで ver を判定する。
//!
//! `git/checkouts` の HEAD は「cargo が fetch / checkout したか」しか表さず、
//! `~/.cargo/bin/<bin>` が実際に置き換わったかとは無関係に先へ進む。実測でも
//! checkout HEAD だけが remote に追いつき、`.crates2.json` と実バイナリの
//! embedded hash は古いまま、という状態が観測された。
//!
//! そのため installed hash の正は「実行バイナリの self-report」とし、この
//! モジュールがその取得を担当する。判定できなかった場合は checkout HEAD へ
//! fallback せず `Unavailable` を返し、呼び出し側で `?`（判定不能）として扱う。
//!
//! `hash` サブコマンドは使わない。実測で `cmrt.exe hash` が引数 `hash` を MML
//! 文字列として解釈しレンダリングを開始したため、副作用のないことが確認できて
//! いる `check` に一本化する。

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// `check` サブコマンドを実装しているとみなす repo owner。
///
/// `self_update::REPO_OWNER` は「cat-repo-auditor 自身の repo owner」という別の
/// 意味を持つ定数なので、値が同じでも流用しない。
pub(crate) const CHECK_SUBCOMMAND_OWNER: &str = "cat2151";

/// 1 本の `<bin> check` を打ち切るまでの時間。
///
/// `check` はアプリ自身が GitHub へ remote HEAD を問い合わせるため、ネットワーク
/// 往復を見込んで長めに取る。
const BIN_CHECK_TIMEOUT: Duration = Duration::from_secs(30);

/// 子プロセスの終了を待つポーリング間隔。
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// installed binary の self-report による ver 判定結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BinCheckOutcome {
    /// `result: up-to-date`
    UpToDate { embedded: String, remote: String },
    /// `result: update available`
    UpdateAvailable { embedded: String, remote: String },
    /// `.crates2.json` に該当 repo の install entry がない（cgo 列は空欄）。
    NotInstalled,
    /// 未実装 / 起動失敗 / 異常終了 / timeout / owner 対象外（cgo 列は `?`）。
    ///
    /// old / ok を断定しないための状態であり、checkout HEAD へ fallback しない。
    Unavailable { reason: String },
}

impl BinCheckOutcome {
    /// ログ末尾に出す完了理由。
    pub(crate) fn completion_label(&self) -> String {
        match self {
            Self::UpToDate { .. } => String::from("binary self-report=up-to-date (ok)"),
            Self::UpdateAvailable { .. } => {
                String::from("binary self-report=update available (NG)")
            }
            Self::NotInstalled => String::from("チェック対象外"),
            Self::Unavailable { reason } => format!("判定不能: {reason}"),
        }
    }

    /// 実バイナリが自己申告した commit hash。判定できなかった場合は `None`。
    pub(crate) fn embedded_hash(&self) -> Option<&str> {
        match self {
            Self::UpToDate { embedded, .. } | Self::UpdateAvailable { embedded, .. } => {
                Some(embedded.as_str())
            }
            Self::NotInstalled | Self::Unavailable { .. } => None,
        }
    }
}

/// `<bin> check` の 3 行出力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedCheck {
    pub(super) embedded: String,
    pub(super) remote: String,
    pub(super) up_to_date: bool,
}

/// `<bin> check` を 1 回実行した結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BinCheckRun {
    pub(super) stdout: String,
    pub(super) stderr: String,
    /// `exit code: 0` のような表示用文字列。
    pub(super) status_label: String,
    pub(super) success: bool,
    pub(super) elapsed: Duration,
}

fn is_commit_hash(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

/// `<bin> check` の stdout を解釈する。
///
/// 期待する形式は次の 3 行。順序は問わず、前後の空白と CRLF を許容する。
///
/// ```text
/// embedded: <40hex>
/// remote: <40hex>
/// result: up-to-date | update available
/// ```
///
/// 3 要素が揃わない場合は `None` を返し、呼び出し側で「`check` 未実装」として扱う。
pub(super) fn parse_check_output(stdout: &str) -> Option<ParsedCheck> {
    let mut embedded: Option<String> = None;
    let mut remote: Option<String> = None;
    let mut up_to_date: Option<bool> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("embedded:") {
            let value = value.trim();
            if is_commit_hash(value) {
                embedded = Some(value.to_string());
            }
        } else if let Some(value) = line.strip_prefix("remote:") {
            let value = value.trim();
            if is_commit_hash(value) {
                remote = Some(value.to_string());
            }
        } else if let Some(value) = line.strip_prefix("result:") {
            up_to_date = match value.trim() {
                "up-to-date" => Some(true),
                "update available" => Some(false),
                _ => None,
            };
        }
    }

    Some(ParsedCheck {
        embedded: embedded?,
        remote: remote?,
        up_to_date: up_to_date?,
    })
}

/// `<bin> check` を実際に起動する。
///
/// TUI を壊さないため stdio は必ず piped / null にする。stdout と stderr は
/// それぞれ別スレッドで読み切り、pipe が詰まって deadlock するのを防ぐ。
fn run_bin_check_command(bin: &str) -> Result<BinCheckRun, String> {
    let started_at = Instant::now();
    let mut child = Command::new(bin)
        .arg("check")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{bin} の起動に失敗しました: {error}"))?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || read_pipe_to_string(stdout_pipe));
    let stderr_reader = std::thread::spawn(move || read_pipe_to_string(stderr_pipe));

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started_at.elapsed() >= BIN_CHECK_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{bin} check が {} 秒で応答しなかったため kill しました",
                        BIN_CHECK_TIMEOUT.as_secs()
                    ));
                }
                std::thread::sleep(WAIT_POLL_INTERVAL);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{bin} check の終了待ちに失敗しました: {error}"));
            }
        }
    };

    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr = stderr_reader.join().unwrap_or_default();

    Ok(BinCheckRun {
        stdout,
        stderr,
        status_label: status.to_string(),
        success: status.success(),
        elapsed: started_at.elapsed(),
    })
}

fn read_pipe_to_string(pipe: Option<impl Read>) -> String {
    let Some(mut pipe) = pipe else {
        return String::new();
    };
    let mut buffer = Vec::new();
    let _ = pipe.read_to_end(&mut buffer);
    String::from_utf8_lossy(&buffer).into_owned()
}

/// installed binary の `check` を実行して ver を判定する。
pub(crate) fn check_installed_bins(owner: &str, repo_name: &str) -> BinCheckOutcome {
    check_installed_bins_with(
        owner,
        repo_name,
        super::append_log_message,
        super::get_cargo_bins_all,
        run_bin_check_command,
    )
}

/// Internal function exposed for testing.
///
/// `list_bins` と `run_check` を注入できるようにし、実プロセスを起動せずに
/// 集約規則と失敗時の扱いを検証できるようにしている。
pub(super) fn check_installed_bins_with<L, B, R>(
    owner: &str,
    repo_name: &str,
    mut log_fn: L,
    mut list_bins: B,
    mut run_check: R,
) -> BinCheckOutcome
where
    L: FnMut(&str),
    B: FnMut(&str, &str) -> Option<Vec<String>>,
    R: FnMut(&str) -> Result<BinCheckRun, String>,
{
    let outcome = resolve_installed_bins_outcome(
        owner,
        repo_name,
        &mut log_fn,
        &mut list_bins,
        &mut run_check,
    );
    super::log_cargo_check_result(
        &mut log_fn,
        owner,
        repo_name,
        &format!(
            "終了: binary self-report check を完了しました ({})",
            outcome.completion_label()
        ),
    );
    outcome
}

fn resolve_installed_bins_outcome<L, B, R>(
    owner: &str,
    repo_name: &str,
    log_fn: &mut L,
    list_bins: &mut B,
    run_check: &mut R,
) -> BinCheckOutcome
where
    L: FnMut(&str),
    B: FnMut(&str, &str) -> Option<Vec<String>>,
    R: FnMut(&str) -> Result<BinCheckRun, String>,
{
    let Some(bins) = list_bins(owner, repo_name) else {
        super::log_cargo_check_result(
            log_fn,
            owner,
            repo_name,
            "cargo install メタデータ内に対象リポジトリが見つからないため、binary self-report check をスキップします",
        );
        return BinCheckOutcome::NotInstalled;
    };

    if !owner.eq_ignore_ascii_case(CHECK_SUBCOMMAND_OWNER) {
        return unavailable(
            log_fn,
            owner,
            repo_name,
            format!(
                "owner が {CHECK_SUBCOMMAND_OWNER} ではないため check サブコマンド実装済みとみなせません"
            ),
        );
    }

    if bins.is_empty() {
        return unavailable(
            log_fn,
            owner,
            repo_name,
            String::from("cargo install メタデータから bin 名を取得できませんでした"),
        );
    }

    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!("binary self-report check の対象 bin={bins:?}"),
    );

    let mut stale: Option<(String, ParsedCheck)> = None;
    let mut first_up_to_date: Option<ParsedCheck> = None;

    for bin in &bins {
        let run = match run_check(bin) {
            Ok(run) => run,
            Err(reason) => return unavailable(log_fn, owner, repo_name, reason),
        };

        log_fn(&format!(
            "cargo check: リポジトリ={owner}/{repo_name} bin={bin} コマンド=`{bin} check` 結果=status={} 標準出力={:?} 標準エラー={:?} 所要={:.1}s",
            run.status_label,
            run.stdout.trim(),
            run.stderr.trim(),
            run.elapsed.as_secs_f64(),
        ));

        if !run.success {
            return unavailable(
                log_fn,
                owner,
                repo_name,
                format!(
                    "{bin} check が異常終了しました (status={})",
                    run.status_label
                ),
            );
        }

        let Some(parsed) = parse_check_output(&run.stdout) else {
            return unavailable(
                log_fn,
                owner,
                repo_name,
                format!("{bin} check の出力を解釈できませんでした (check 未実装とみなします)"),
            );
        };

        if parsed.up_to_date {
            if first_up_to_date.is_none() {
                first_up_to_date = Some(parsed);
            }
        } else if stale.is_none() {
            stale = Some((bin.clone(), parsed));
        }
    }

    // 1 本でも古ければ repo 全体を NG とする。workspace から複数 bin が入る repo で、
    // 片方だけ置き換わらなかった状態を取りこぼさないため。
    if let Some((bin, parsed)) = stale {
        super::log_cargo_check_result(
            log_fn,
            owner,
            repo_name,
            &format!(
                "binary self-report を採用: bin={bin} embedded={} remote={} 判定=update available (NG)",
                parsed.embedded, parsed.remote
            ),
        );
        return BinCheckOutcome::UpdateAvailable {
            embedded: parsed.embedded,
            remote: parsed.remote,
        };
    }

    let parsed = first_up_to_date
        .expect("bins が空でなく全 bin が成功したので up-to-date が 1 件以上ある想定です");
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!(
            "binary self-report を採用: embedded={} remote={} 判定=up-to-date (ok)",
            parsed.embedded, parsed.remote
        ),
    );
    BinCheckOutcome::UpToDate {
        embedded: parsed.embedded,
        remote: parsed.remote,
    }
}

fn unavailable<L>(log_fn: &mut L, owner: &str, repo_name: &str, reason: String) -> BinCheckOutcome
where
    L: FnMut(&str),
{
    super::log_cargo_check_result(
        log_fn,
        owner,
        repo_name,
        &format!("binary self-report check ができませんでした: {reason}"),
    );
    BinCheckOutcome::Unavailable { reason }
}

#[cfg(test)]
#[path = "github_local_cargo_bin_check_tests.rs"]
mod tests;
