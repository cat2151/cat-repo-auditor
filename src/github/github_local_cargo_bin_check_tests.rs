use super::*;

const EMBEDDED: &str = "4ecf42e931e9dc3af0dd89bd53351676a2899a23";
const REMOTE: &str = "47049c0fe70d57e233d8943a4abab5bf780621bc";

fn ok_run(stdout: &str) -> BinCheckRun {
    BinCheckRun {
        stdout: stdout.to_string(),
        stderr: String::new(),
        status_label: String::from("exit code: 0"),
        success: true,
        elapsed: Duration::from_millis(800),
    }
}

fn check_stdout(embedded: &str, remote: &str) -> String {
    let result = if embedded == remote {
        "up-to-date"
    } else {
        "update available"
    };
    format!("embedded: {embedded}\nremote: {remote}\nresult: {result}")
}

fn bins(names: &[&str]) -> Option<Vec<String>> {
    Some(names.iter().map(|name| name.to_string()).collect())
}

fn no_log(_: &str) {}

#[test]
fn parse_check_output_reads_the_three_line_format() {
    let parsed = parse_check_output(&check_stdout(EMBEDDED, REMOTE)).unwrap();
    assert_eq!(
        parsed,
        ParsedCheck {
            embedded: EMBEDDED.to_string(),
            remote: REMOTE.to_string(),
            up_to_date: false,
        }
    );
}

#[test]
fn parse_check_output_reads_up_to_date() {
    let parsed = parse_check_output(&check_stdout(EMBEDDED, EMBEDDED)).unwrap();
    assert!(parsed.up_to_date);
    assert_eq!(parsed.embedded, EMBEDDED);
    assert_eq!(parsed.remote, EMBEDDED);
}

#[test]
fn parse_check_output_allows_crlf_and_surrounding_whitespace() {
    let stdout =
        format!("  embedded:   {EMBEDDED} \r\nremote: {REMOTE}\r\nresult:  update available \r\n");
    let parsed = parse_check_output(&stdout).unwrap();
    assert_eq!(parsed.embedded, EMBEDDED);
    assert_eq!(parsed.remote, REMOTE);
    assert!(!parsed.up_to_date);
}

#[test]
fn parse_check_output_ignores_unrelated_extra_lines() {
    let stdout = format!(
        "config.toml を読み込みました\nembedded: {EMBEDDED}\nremote: {REMOTE}\nresult: update available\nbye\n"
    );
    assert!(parse_check_output(&stdout).is_some());
}

#[test]
fn parse_check_output_rejects_missing_lines() {
    let stdout = format!("embedded: {EMBEDDED}\nresult: up-to-date");
    assert_eq!(parse_check_output(&stdout), None);
}

#[test]
fn parse_check_output_rejects_non_hash_values() {
    let stdout = format!("embedded: unknown\nremote: {REMOTE}\nresult: update available");
    assert_eq!(parse_check_output(&stdout), None);
}

#[test]
fn parse_check_output_rejects_unknown_result_word() {
    let stdout = format!("embedded: {EMBEDDED}\nremote: {REMOTE}\nresult: maybe");
    assert_eq!(parse_check_output(&stdout), None);
}

#[test]
fn parse_check_output_rejects_empty_output() {
    assert_eq!(parse_check_output(""), None);
}

#[test]
fn parse_check_output_rejects_unrecognized_subcommand_output() {
    // `hash` を持たないアプリが返す形。3 要素が揃わないので未実装扱いになる。
    assert_eq!(
        parse_check_output("error: unrecognized subcommand 'check'"),
        None
    );
}

#[test]
fn check_installed_bins_reports_up_to_date_when_every_bin_matches() {
    let outcome = check_installed_bins_with(
        "cat2151",
        "clap-mml-play-server",
        no_log,
        |_, _| {
            bins(&[
                "clap-mml-render-server.exe",
                "clap-mml-realtime-play-server.exe",
            ])
        },
        |_| Ok(ok_run(&check_stdout(EMBEDDED, EMBEDDED))),
    );
    assert_eq!(
        outcome,
        BinCheckOutcome::UpToDate {
            embedded: EMBEDDED.to_string(),
            remote: EMBEDDED.to_string(),
        }
    );
}

#[test]
fn check_installed_bins_reports_ng_when_any_bin_is_stale() {
    // workspace で 2 本入る repo のうち、片方だけ置き換わらなかった状態。
    let outcome = check_installed_bins_with(
        "cat2151",
        "clap-mml-play-server",
        no_log,
        |_, _| {
            bins(&[
                "clap-mml-render-server.exe",
                "clap-mml-realtime-play-server.exe",
            ])
        },
        |bin| {
            if bin == "clap-mml-realtime-play-server.exe" {
                Ok(ok_run(&check_stdout(EMBEDDED, REMOTE)))
            } else {
                Ok(ok_run(&check_stdout(REMOTE, REMOTE)))
            }
        },
    );
    assert_eq!(
        outcome,
        BinCheckOutcome::UpdateAvailable {
            embedded: EMBEDDED.to_string(),
            remote: REMOTE.to_string(),
        }
    );
}

#[test]
fn check_installed_bins_is_not_installed_without_a_cargo_entry() {
    let outcome = check_installed_bins_with(
        "cat2151",
        "not-installed",
        no_log,
        |_, _| None,
        |_| panic!("install entry が無いので bin は起動しない想定です"),
    );
    assert_eq!(outcome, BinCheckOutcome::NotInstalled);
}

#[test]
fn check_installed_bins_is_unavailable_for_other_owners() {
    let outcome = check_installed_bins_with(
        "someone-else",
        "some-repo",
        no_log,
        |_, _| bins(&["some-repo.exe"]),
        |_| panic!("owner が対象外なので bin は起動しない想定です"),
    );
    assert!(matches!(outcome, BinCheckOutcome::Unavailable { .. }));
}

#[test]
fn check_installed_bins_is_unavailable_when_launch_fails() {
    let outcome = check_installed_bins_with(
        "cat2151",
        "cat-task-manager",
        no_log,
        |_, _| bins(&["cat-task-manager.exe"]),
        |_| Err(String::from("起動に失敗しました")),
    );
    assert!(matches!(outcome, BinCheckOutcome::Unavailable { .. }));
}

#[test]
fn check_installed_bins_is_unavailable_on_timeout() {
    let outcome = check_installed_bins_with(
        "cat2151",
        "cat-task-manager",
        no_log,
        |_, _| bins(&["cat-task-manager.exe"]),
        |bin| {
            Err(format!(
                "{bin} check が 30 秒で応答しなかったため kill しました"
            ))
        },
    );
    let BinCheckOutcome::Unavailable { reason } = outcome else {
        panic!("timeout は Unavailable になる想定です");
    };
    assert!(reason.contains("kill"));
}

#[test]
fn check_installed_bins_is_unavailable_when_the_subcommand_is_missing() {
    let outcome = check_installed_bins_with(
        "cat2151",
        "some-repo",
        no_log,
        |_, _| bins(&["some-repo.exe"]),
        |_| {
            Ok(BinCheckRun {
                stdout: String::new(),
                stderr: String::from("error: unrecognized subcommand 'check'"),
                status_label: String::from("exit code: 2"),
                success: false,
                elapsed: Duration::from_millis(10),
            })
        },
    );
    assert!(matches!(outcome, BinCheckOutcome::Unavailable { .. }));
}

#[test]
fn check_installed_bins_is_unavailable_when_output_is_unparsable() {
    let outcome = check_installed_bins_with(
        "cat2151",
        "some-repo",
        no_log,
        |_, _| bins(&["some-repo.exe"]),
        |_| Ok(ok_run("CLI モード: MML = check")),
    );
    assert!(matches!(outcome, BinCheckOutcome::Unavailable { .. }));
}

#[test]
fn check_installed_bins_stops_at_the_first_failing_bin() {
    // 1 本目が失敗したら残りは起動しない（無駄なネットワークアクセスを避ける）。
    let mut launched: Vec<String> = Vec::new();
    let outcome = check_installed_bins_with(
        "cat2151",
        "clap-mml-play-server",
        no_log,
        |_, _| bins(&["first.exe", "second.exe"]),
        |bin| {
            launched.push(bin.to_string());
            Err(String::from("起動に失敗しました"))
        },
    );
    assert!(matches!(outcome, BinCheckOutcome::Unavailable { .. }));
    assert_eq!(launched, vec![String::from("first.exe")]);
}

#[test]
fn check_installed_bins_is_unavailable_when_the_entry_has_no_bins() {
    let outcome = check_installed_bins_with(
        "cat2151",
        "some-repo",
        no_log,
        |_, _| Some(vec![]),
        |_| panic!("bin 名が無いので起動しない想定です"),
    );
    assert!(matches!(outcome, BinCheckOutcome::Unavailable { .. }));
}

/// 実機の cargo install 状態に対して `check` を本当に起動する実測用テスト。
///
/// 机上チェックだけで済ませないための確認手段（AGENTS.md）。network と実機の
/// インストール状態に依存するので通常の `cargo test` からは外してある。
///
/// ```text
/// cargo test -- --ignored --nocapture bin_check_against_the_real_installed_binaries
/// CATREPO_BIN_CHECK_REPO=cat-task-manager cargo test -- --ignored --nocapture bin_check_against_the_real_installed_binaries
/// ```
#[test]
#[ignore = "実機の cargo install 状態と network に依存する実測テスト"]
fn bin_check_against_the_real_installed_binaries() {
    let repo = std::env::var("CATREPO_BIN_CHECK_REPO")
        .unwrap_or_else(|_| String::from("cat-repo-auditor"));
    let outcome = super::check_installed_bins(CHECK_SUBCOMMAND_OWNER, &repo);
    println!("{CHECK_SUBCOMMAND_OWNER}/{repo} => {outcome:?}");

    assert!(
        !matches!(outcome, BinCheckOutcome::Unavailable { .. }),
        "cargo install 済みの cat2151 アプリは check サブコマンドで判定できる想定です: {outcome:?}"
    );
}
