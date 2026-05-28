//! Integration tests for `state-backfill` subcmd (P008).
//!
//! Sub-mech C verification: pre.seen_advisories MUST be subset of post.
//! Sub-mech F verification: --dry-run must not modify state file (byte-identity).

use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str::contains;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Parse state file as serde_json::Value for field-level assertions.
fn read_state_value(path: &std::path::Path) -> serde_json::Value {
    let content = std::fs::read_to_string(path).expect("read state file");
    serde_json::from_str(&content).expect("parse state JSON")
}

/// Extract seen_advisories array from state Value as sorted Vec<String>.
fn seen_advisories(v: &serde_json::Value) -> Vec<String> {
    v["seen_advisories"]
        .as_array()
        .expect("seen_advisories array")
        .iter()
        .map(|id| id.as_str().expect("string ID").to_string())
        .collect()
}

/// Test A — Acceptance: 5 rows, 3 processed/dismissed + state 1 ID → 4 IDs.
/// Also covers Test D (open rows excluded) per phiếu spec.
#[test]
fn acceptance_5_rows_3_processed_plus_state_1id_produces_4_ids() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");
    let inbox_path = tmp.path().join("inbox.md");
    std::fs::copy(fixtures_dir().join("state-1id.json"), &state_path).unwrap();
    std::fs::copy(
        fixtures_dir().join("inbox-5rows-3processed.md"),
        &inbox_path,
    )
    .unwrap();

    let pre = read_state_value(&state_path);

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "state-backfill",
            "--state",
            state_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("\"backfilled_count\":3"))
        .stdout(contains("\"total_seen_after\":4"));

    let post = read_state_value(&state_path);
    let pre_ids = seen_advisories(&pre);
    let post_ids = seen_advisories(&post);

    // Sub-mech C: all pre IDs must survive backfill.
    for id in &pre_ids {
        assert!(
            post_ids.contains(id),
            "Sub-mech C violation: pre ID {} lost in backfill",
            id
        );
    }
    // Sub-mech C: post count >= pre count.
    assert!(
        post_ids.len() >= pre_ids.len(),
        "Sub-mech C: seen_advisories must not shrink"
    );
    // Acceptance: exactly 4 IDs (1 pre + 3 new processed/dismissed).
    assert_eq!(post_ids.len(), 4);
    // Pre ID preserved.
    assert!(post_ids.contains(&"CVE-2026-7777".to_string()));
    // 3 backfilled IDs present.
    assert!(post_ids.contains(&"CVE-2026-9001".to_string()));
    assert!(post_ids.contains(&"CVE-2026-9002".to_string()));
    assert!(post_ids.contains(&"CVE-2026-9003".to_string()));
    // 2 open rows did NOT contribute (Test D — explicit exclusion check).
    assert!(!post_ids.contains(&"CVE-2026-9004".to_string()));
    assert!(!post_ids.contains(&"CVE-2026-9005".to_string()));
    // Sub-mech C: last_scan_at PRESERVED (backfill is not a scan event).
    assert_eq!(
        post["last_scan_at"].as_str().unwrap(),
        pre["last_scan_at"].as_str().unwrap(),
        "last_scan_at must be preserved (not a scan event)"
    );
    // agent_version PRESERVED.
    assert_eq!(
        post["agent_version"].as_str().unwrap(),
        pre["agent_version"].as_str().unwrap(),
        "agent_version must be preserved"
    );
}

/// Test B — Already-backfilled (no new IDs): backfilled_count = 0, file stable.
#[test]
fn already_backfilled_zero_count() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");
    let inbox_path = tmp.path().join("inbox.md");
    // Pre-populate state with all 4 expected IDs already present.
    let pre_state_json = serde_json::json!({
        "schema_version": 1,
        "last_scan_at": "2026-05-23T12:00:00Z",
        "seen_advisories": [
            "CVE-2026-7777",
            "CVE-2026-9001",
            "CVE-2026-9002",
            "CVE-2026-9003"
        ],
        "agent_version": "advisory-watch@0.1.0"
    });
    std::fs::write(
        &state_path,
        serde_json::to_string_pretty(&pre_state_json).unwrap() + "\n",
    )
    .unwrap();
    std::fs::copy(
        fixtures_dir().join("inbox-5rows-3processed.md"),
        &inbox_path,
    )
    .unwrap();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "state-backfill",
            "--state",
            state_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("\"backfilled_count\":0"))
        .stdout(contains("\"total_seen_after\":4"));

    let post = read_state_value(&state_path);
    // Still 4 IDs after idempotent re-write.
    assert_eq!(seen_advisories(&post).len(), 4);
}

/// Test C — `--dry-run` byte-identity: state file bytes unchanged (Sub-mech F).
#[test]
fn dry_run_does_not_touch_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");
    let inbox_path = tmp.path().join("inbox.md");
    std::fs::copy(fixtures_dir().join("state-1id.json"), &state_path).unwrap();
    std::fs::copy(
        fixtures_dir().join("inbox-5rows-3processed.md"),
        &inbox_path,
    )
    .unwrap();

    let pre_bytes = std::fs::read(&state_path).unwrap();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "state-backfill",
            "--state",
            state_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("\"backfilled_count\":3"))
        .stdout(contains("\"total_seen_after\":4"));

    let post_bytes = std::fs::read(&state_path).unwrap();
    assert_eq!(
        pre_bytes, post_bytes,
        "Sub-mech F: --dry-run must NOT modify state file (byte-identity contract)"
    );
}

/// Test D — Open rows explicitly excluded (standalone; also covered in Test A).
#[test]
fn only_processed_and_dismissed_contribute() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");
    let inbox_path = tmp.path().join("inbox.md");
    std::fs::copy(fixtures_dir().join("state-1id.json"), &state_path).unwrap();
    std::fs::copy(
        fixtures_dir().join("inbox-5rows-3processed.md"),
        &inbox_path,
    )
    .unwrap();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "state-backfill",
            "--state",
            state_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let post = read_state_value(&state_path);
    let post_ids = seen_advisories(&post);
    // Open rows from fixture: CVE-2026-9004, CVE-2026-9005 must NOT appear.
    assert!(
        !post_ids.contains(&"CVE-2026-9004".to_string()),
        "open row CVE-2026-9004 must not be backfilled"
    );
    assert!(
        !post_ids.contains(&"CVE-2026-9005".to_string()),
        "open row CVE-2026-9005 must not be backfilled"
    );
}
