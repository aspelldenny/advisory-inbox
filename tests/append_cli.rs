//! Integration tests for `advisory-inbox append` subcmd.
//!
//! Covers: happy path (2 new rows inserted at top, old rows preserved,
//! placeholder comment intact, exit 0), missing `## Rows` heading (exit 1),
//! rows JSON malformed (exit 2), atomic-write smoke test (file exists post-persist).

use std::io::Write;

use assert_cmd::Command;
use predicates::prelude::*;

const BASELINE_FIXTURE: &str = "tests/fixtures/inbox-baseline.md";
const ROWS_2: &str = "tests/fixtures/rows-2.json";

#[test]
fn append_happy_path_2_new_rows() {
    // Copy baseline to a tempdir so the test mutates a throwaway file.
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("inbox.md");
    std::fs::copy(BASELINE_FIXTURE, &target).expect("copy baseline");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("append")
        .arg("--inbox")
        .arg(&target)
        .arg("--rows-json")
        .arg(ROWS_2)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""appended_count":2"#))
        .stdout(predicate::str::contains(r#""total_open":3"#));

    // Verify file content post-append.
    let after = std::fs::read_to_string(&target).expect("read after");

    // New rows present.
    assert!(after.contains("CVE-NEW-1"), "CVE-NEW-1 missing");
    assert!(after.contains("CVE-NEW-2"), "CVE-NEW-2 missing");
    // Old rows preserved.
    assert!(after.contains("CVE-OLD-1"), "CVE-OLD-1 missing");
    assert!(after.contains("CVE-OLD-2"), "CVE-OLD-2 missing");
    // Placeholder HTML comment block intact.
    assert!(
        after.contains("GHSA-placeholder"),
        "placeholder comment block damaged"
    );
    assert!(
        after.contains("<!-- Placeholder example"),
        "placeholder header comment damaged"
    );

    // Order: new rows appear BEFORE old rows in the inbox text.
    let pos_new1 = after.find("CVE-NEW-1").expect("new1 pos");
    let pos_old1 = after.find("CVE-OLD-1").expect("old1 pos");
    assert!(pos_new1 < pos_old1, "newest rows must be at top of ## Rows");

    // `## Rows` heading still present and appears exactly ONCE.
    assert_eq!(
        after.matches("## Rows").count(),
        1,
        "## Rows heading should be unique"
    );
}

#[test]
fn append_missing_heading_exit_1() {
    // Inline-build an inbox without `## Rows`.
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("inbox-noheading.md");
    std::fs::write(&target, "# Advisory Inbox\n\nNo heading here.\n").expect("write");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("append")
        .arg("--inbox")
        .arg(&target)
        .arg("--rows-json")
        .arg(ROWS_2)
        .assert()
        .failure()
        .code(1)
        .stderr(
            predicate::str::contains("## Rows")
                .or(predicate::str::contains("heading"))
                .or(predicate::str::contains("missing")),
        );
}

#[test]
fn append_rows_malformed_exit_2() {
    // Inline-build a rows file without the "rows" key.
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("inbox.md");
    std::fs::copy(BASELINE_FIXTURE, &target).expect("copy baseline");

    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    tmp.write_all(br#"{"not_rows": []}"#).expect("write tmp");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("append")
        .arg("--inbox")
        .arg(&target)
        .arg("--rows-json")
        .arg(tmp.path())
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("rows").or(predicate::str::contains("JSON")));
}

#[test]
fn append_atomic_write_no_leftover_tmp() {
    // Smoke test: after successful append, no leftover temp file in parent dir.
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("inbox.md");
    std::fs::copy(BASELINE_FIXTURE, &target).expect("copy baseline");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("append")
        .arg("--inbox")
        .arg(&target)
        .arg("--rows-json")
        .arg(ROWS_2)
        .assert()
        .success();

    // After persist, the temp file is renamed to target — no sibling .tmp* remains.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("readdir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // Expect exactly 1 entry: "inbox.md". No .tmpXXXX siblings.
    assert_eq!(entries.len(), 1, "leftover temp files: {entries:?}");
    assert_eq!(entries[0], "inbox.md");
}
