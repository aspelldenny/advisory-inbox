//! Integration tests for `advisory-inbox migrate-state` subcommand.

use std::path::PathBuf;

use assert_cmd::Command;
use chrono::{DateTime, Utc};
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

/// Copy a fixture file into a tempdir and return the destination path.
fn copy_fixture_to_tempdir(
    fixture_name: &str,
    dir: &tempfile::TempDir,
    target_name: &str,
) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture_name);
    let dest = dir.path().join(target_name);
    std::fs::copy(&src, &dest).expect("copy fixture");
    dest
}

/// Test A — Missing file: creates fresh JSON v1 at target path.
#[test]
fn migrate_missing_writes_fresh_json_v1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("state.json");
    assert!(!target.exists(), "precondition: target should not exist");

    Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .assert()
        .success()
        .stdout(contains("\"from\""))
        .stdout(contains("missing"))
        .stdout(contains("\"to\""))
        .stdout(contains("json-v1"))
        .stdout(contains("\"seen_count\""))
        .stdout(contains("0"));

    // File now exists with valid JSON v1.
    assert!(target.exists(), "target should be created");
    let content = std::fs::read_to_string(&target).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
    assert_eq!(parsed["schema_version"], 1);
    assert!(parsed["last_scan_at"].is_string());
    assert_eq!(parsed["seen_advisories"].as_array().unwrap().len(), 0);
}

/// Test B — Legacy: preserves timestamp (Sub-mech C migration completeness check).
#[test]
fn migrate_legacy_preserves_timestamp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = copy_fixture_to_tempdir("state-legacy.txt", &dir, "state");

    Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .assert()
        .success()
        .stdout(contains("\"from\""))
        .stdout(contains("legacy"))
        .stdout(contains("\"seen_count\""))
        .stdout(contains("0"));

    // SUB-MECH C — timestamp must survive legacy → JSON v1 conversion.
    let migrated = std::fs::read_to_string(&target).expect("read migrated");
    let parsed: serde_json::Value = serde_json::from_str(&migrated).expect("parse JSON");
    let last_scan_at = parsed["last_scan_at"]
        .as_str()
        .expect("last_scan_at string");
    let actual_dt = DateTime::parse_from_rfc3339(last_scan_at).expect("parse rfc3339");
    let expected_dt = DateTime::parse_from_rfc3339("2026-05-23T12:00:00Z")
        .expect("parse expected")
        .with_timezone(&Utc);
    assert_eq!(
        actual_dt.with_timezone(&Utc),
        expected_dt,
        "timestamp must survive migration (Sub-mech C)"
    );
}

/// Test C — JSON v1 already: idempotent re-write, seen_count preserved.
#[test]
fn migrate_json_v1_idempotent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = copy_fixture_to_tempdir("state-json-v1.json", &dir, "state.json");

    Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .assert()
        .success()
        .stdout(contains("\"from\""))
        .stdout(contains("json-v1"))
        .stdout(contains("\"seen_count\""))
        .stdout(contains("2"));

    // File still parses as JSON v1 with seen_count = 2.
    let migrated = std::fs::read_to_string(&target).expect("read migrated");
    let parsed: serde_json::Value = serde_json::from_str(&migrated).expect("parse JSON");
    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["seen_advisories"].as_array().unwrap().len(), 2);
}

/// Test D — Garbage: exit 1, stderr hints format/ISO, file unchanged.
#[test]
fn migrate_garbage_errors_exit_1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = copy_fixture_to_tempdir("state-garbage.txt", &dir, "state");
    let before = std::fs::read_to_string(&target).expect("read before");

    Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .assert()
        .failure()
        .code(1)
        .stderr(contains("format").or(contains("ISO")));

    // File content unchanged — no partial-write, no overwrite.
    let after = std::fs::read_to_string(&target).expect("read after");
    assert_eq!(before, after, "garbage input should leave file untouched");
}

/// Test E — `--dry-run` legacy: exit 0, file content UNCHANGED, no tmp artifact.
#[test]
fn migrate_dry_run_legacy_no_file_change() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = copy_fixture_to_tempdir("state-legacy.txt", &dir, "state");
    let before = std::fs::read_to_string(&target).expect("read before");

    Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(contains("\"from\""))
        .stdout(contains("legacy"));

    // File content UNCHANGED on disk.
    let after = std::fs::read_to_string(&target).expect("read after");
    assert_eq!(before, after, "dry-run must not touch file");

    // No .tmp leftover in tempdir.
    let leftover = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".tmp")
        })
        .count();
    assert_eq!(leftover, 0, "no temp artifact should remain after dry-run");
}
