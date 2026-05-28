//! Integration tests for `advisory-inbox parse-report` subcmd.
//!
//! Covers: happy path (fixture file via stdin), missing sentinel exit 1,
//! bad row format exit 2.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn parse_report_happy_path_two_rows() {
    let fixture = include_str!("fixtures/agent-report-1.md");
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("parse-report")
        .write_stdin(fixture)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""advisories_found":2"#))
        .stdout(predicate::str::contains("CVE-2026-9999"))
        .stdout(predicate::str::contains("GHSA-aaaa-bbbb"));
}

#[test]
fn parse_report_missing_sentinel_exit_1() {
    let input = "just some prose without any sentinel markers at all\n";
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("parse-report")
        .write_stdin(input)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("missing sentinel start marker"));
}

#[test]
fn parse_report_bad_severity_exit_2() {
    let input = "\
<!-- INBOX_APPEND_START -->
| 2026-05-28 | CVE-X | u | p | f:1 | Critic | open | - |
<!-- INBOX_APPEND_END -->
";
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("parse-report")
        .write_stdin(input)
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("invalid severity"));
}
