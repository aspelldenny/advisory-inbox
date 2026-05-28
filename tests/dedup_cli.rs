//! Integration tests for `advisory-inbox dedup` subcmd.
//!
//! Covers: happy path (3 kept + 2 skipped + 5 observed_ids), state file missing
//! (exit 1), schema_version mismatch (exit 1), rows JSON malformed (exit 2).

use std::io::Write as _;

use assert_cmd::Command;
use predicates::prelude::*;

const STATE_3IDS: &str = "tests/fixtures/state-3ids.json";
const ROWS_5: &str = "tests/fixtures/rows-5.json";

#[test]
fn dedup_happy_path_3_kept_2_skipped() {
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("dedup")
        .arg("--state")
        .arg(STATE_3IDS)
        .arg("--rows-json")
        .arg(ROWS_5)
        .assert()
        .success()
        // 3 kept: CVE-2026-NEW1, CVE-2026-NEW2, CVE-2026-NEW3
        .stdout(predicate::str::contains("CVE-2026-NEW1"))
        .stdout(predicate::str::contains("CVE-2026-NEW2"))
        .stdout(predicate::str::contains("CVE-2026-NEW3"))
        // 2 skipped: CVE-2026-9256, GHSA-aaaa-bbbb
        .stdout(predicate::str::contains("CVE-2026-9256"))
        .stdout(predicate::str::contains("GHSA-aaaa-bbbb"))
        // observed_ids includes all 5 (JSON key match)
        .stdout(predicate::str::contains(r#""observed_ids""#))
        .stdout(predicate::str::contains(r#""kept""#))
        .stdout(predicate::str::contains(r#""skipped""#));
}

#[test]
fn dedup_state_missing_exit_1() {
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("dedup")
        .arg("--state")
        .arg("/nonexistent/advisory-state.json")
        .arg("--rows-json")
        .arg(ROWS_5)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("state file"));
}

#[test]
fn dedup_state_schema_mismatch_exit_1() {
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let bad_state = r#"{
        "schema_version": 99,
        "last_scan_at": "2026-05-28T09:51:35Z",
        "seen_advisories": [],
        "agent_version": "x"
    }"#;
    tmp.write_all(bad_state.as_bytes()).expect("write tmp");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("dedup")
        .arg("--state")
        .arg(tmp.path())
        .arg("--rows-json")
        .arg(ROWS_5)
        .assert()
        .failure()
        .code(1)
        .stderr(
            predicate::str::contains("schema_version")
                .or(predicate::str::contains("migrate-state")),
        );
}

#[test]
fn dedup_rows_malformed_exit_2() {
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    tmp.write_all(br#"{"not_rows": []}"#).expect("write tmp");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("dedup")
        .arg("--state")
        .arg(STATE_3IDS)
        .arg("--rows-json")
        .arg(tmp.path())
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("rows"));
}
