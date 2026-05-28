//! Integration tests for `scan-and-append` composite subcmd (P009).
//!
//! Sub-mech C verification: state.seen_advisories monotonic non-shrink across composite invocation.
//! Sub-mech F verification: write order is inbox first then state (enforced in implementation;
//! verified here via post-condition assertions on both files).
//!
//! Fixture setup: `agent-report-1.md` yields 2 rows:
//!   - CVE-2026-9999 (High / open)
//!   - GHSA-aaaa-bbbb (Medium / open)
//!
//! `state-3ids.json` pre-seen: [CVE-2026-9256, GHSA-aaaa-bbbb, CVE-2026-27205].
//! Overlap with report: GHSA-aaaa-bbbb → 1 skipped, CVE-2026-9999 → 1 kept.

use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str::contains;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Parse state JSON file as Value for field-level assertions.
fn read_state(path: &std::path::Path) -> serde_json::Value {
    let content = std::fs::read_to_string(path).expect("read state file");
    serde_json::from_str(&content).expect("parse state JSON")
}

/// Extract seen_advisories from state Value as Vec<String>.
fn seen_ids(v: &serde_json::Value) -> Vec<String> {
    v["seen_advisories"]
        .as_array()
        .expect("seen_advisories array")
        .iter()
        .map(|id| id.as_str().expect("string id").to_string())
        .collect()
}

/// Test A — Happy end-to-end: 2 rows in report, 1 overlap with state → 1 kept, 1 skipped.
///
/// Fixture plan (Anchor #22):
///
/// - state-3ids.json has GHSA-aaaa-bbbb → 1 skip.
/// - CVE-2026-9999 not in state-3ids.json → 1 kept.
/// - Expected: appended=1, skipped_dedup=1, total_open=K (≥1).
///
/// Sub-mech C invariants verified:
///   - All pre IDs survive in post.
///   - post.seen_advisories.len() >= pre.seen_advisories.len().
///   - post.last_scan_at > pre.last_scan_at (scan event).
///   - post.agent_version == pre.agent_version (preserved).
///   - post.schema_version == 1 (preserved).
#[test]
fn acceptance_end_to_end_one_kept_one_skipped() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let report_path = tmp.path().join("report.md");
    let inbox_path = tmp.path().join("inbox.md");
    let state_path = tmp.path().join("state.json");

    std::fs::copy(fixtures_dir().join("agent-report-1.md"), &report_path).unwrap();
    std::fs::copy(fixtures_dir().join("inbox-baseline.md"), &inbox_path).unwrap();
    // Build state inline with a past timestamp so post.last_scan_at > pre is guaranteed.
    // IDs mirror state-3ids.json content: GHSA-aaaa-bbbb overlaps report (1 skip);
    // CVE-2026-9999 not present → 1 kept.
    let pre_state_json = serde_json::json!({
        "schema_version": 1,
        "last_scan_at": "2026-01-01T00:00:00Z",
        "seen_advisories": ["CVE-2026-9256", "GHSA-aaaa-bbbb", "CVE-2026-27205"],
        "agent_version": "advisory-watch@0.1.0"
    });
    std::fs::write(
        &state_path,
        serde_json::to_string_pretty(&pre_state_json).unwrap() + "\n",
    )
    .unwrap();

    let pre = read_state(&state_path);
    let pre_ids = seen_ids(&pre);
    let pre_last_scan = pre["last_scan_at"].as_str().unwrap().to_string();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "scan-and-append",
            "--report",
            report_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("\"appended\":1"))
        .stdout(contains("\"skipped_dedup\":1"))
        .stdout(contains("\"total_open\":"));

    let post = read_state(&state_path);
    let post_ids = seen_ids(&post);

    // Sub-mech C: every pre ID survives in post.
    for id in &pre_ids {
        assert!(
            post_ids.contains(id),
            "Sub-mech C violation: pre ID '{}' lost from post.seen_advisories",
            id
        );
    }
    // Monotonic non-shrink.
    assert!(
        post_ids.len() >= pre_ids.len(),
        "post.seen_advisories.len() {} < pre {} — shrink violation",
        post_ids.len(),
        pre_ids.len()
    );
    // CVE-2026-9999 now present (was kept).
    assert!(
        post_ids.contains(&"CVE-2026-9999".to_string()),
        "CVE-2026-9999 must appear in post.seen_advisories"
    );

    // last_scan_at BUMPED (post timestamp > pre fixture "2026-05-28T09:51:35Z").
    let post_last_scan = post["last_scan_at"].as_str().unwrap().to_string();
    assert!(
        post_last_scan > pre_last_scan,
        "post.last_scan_at '{}' must be > pre '{}'",
        post_last_scan,
        pre_last_scan
    );

    // agent_version PRESERVED.
    assert_eq!(
        post["agent_version"].as_str().unwrap(),
        pre["agent_version"].as_str().unwrap()
    );
    // schema_version PRESERVED == 1.
    assert_eq!(post["schema_version"].as_u64().unwrap(), 1);

    // Inbox post-condition: ## Rows heading present; CVE-2026-9999 row inserted.
    let inbox_post = std::fs::read_to_string(&inbox_path).expect("read inbox post");
    assert!(
        inbox_post.contains("## Rows"),
        "## Rows heading must survive"
    );
    assert!(
        inbox_post.contains("CVE-2026-9999"),
        "kept row CVE-2026-9999 must appear in inbox"
    );
    // Existing baseline rows preserved.
    assert!(
        inbox_post.contains("CVE-OLD-1"),
        "pre-existing CVE-OLD-1 must be preserved"
    );
}

/// Test B — All skipped: state pre-populated with ALL advisory_ids from report → 0 appended.
///
/// Uses `parse-report --input` to extract IDs at runtime (robust to fixture changes).
#[test]
fn all_skipped_full_overlap_with_state() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let report_path = tmp.path().join("report.md");
    let inbox_path = tmp.path().join("inbox.md");
    let state_path = tmp.path().join("state.json");

    std::fs::copy(fixtures_dir().join("agent-report-1.md"), &report_path).unwrap();
    std::fs::copy(fixtures_dir().join("inbox-baseline.md"), &inbox_path).unwrap();

    // Extract all advisory_ids from report via parse-report subcommand.
    let parse_out = Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args(["parse-report", "--input", report_path.to_str().unwrap()])
        .output()
        .expect("parse-report invocation");
    assert!(parse_out.status.success(), "parse-report must succeed");
    let parsed: serde_json::Value =
        serde_json::from_slice(&parse_out.stdout).expect("parse-report JSON");
    let report_ids: Vec<String> = parsed["rows"]
        .as_array()
        .expect("rows array")
        .iter()
        .map(|r| r["advisory_id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !report_ids.is_empty(),
        "agent-report-1.md must yield ≥1 row"
    );

    // Build state with ALL report IDs pre-seen.
    let pre_state = serde_json::json!({
        "schema_version": 1,
        "last_scan_at": "2026-05-23T12:00:00Z",
        "seen_advisories": report_ids,
        "agent_version": "advisory-watch@0.1.0"
    });
    std::fs::write(
        &state_path,
        serde_json::to_string_pretty(&pre_state).unwrap() + "\n",
    )
    .unwrap();

    let pre = read_state(&state_path);
    let pre_ids = seen_ids(&pre);
    let pre_last_scan = pre["last_scan_at"].as_str().unwrap().to_string();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "scan-and-append",
            "--report",
            report_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("\"appended\":0"))
        .stdout(contains(format!("\"skipped_dedup\":{}", pre_ids.len())));

    let post = read_state(&state_path);
    let post_ids = seen_ids(&post);

    // seen_advisories count unchanged (all IDs already in state).
    assert_eq!(
        post_ids.len(),
        pre_ids.len(),
        "all-skipped: seen_advisories count must be unchanged"
    );
    // Sub-mech C: last_scan_at still BUMPED (scan event even if nothing kept).
    let post_last_scan = post["last_scan_at"].as_str().unwrap().to_string();
    assert!(
        post_last_scan > pre_last_scan,
        "last_scan_at must be bumped even when 0 rows appended"
    );
    // Inbox: ## Rows heading still present.
    let inbox_post = std::fs::read_to_string(&inbox_path).expect("read inbox post");
    assert!(
        inbox_post.contains("## Rows"),
        "## Rows heading must survive"
    );
}

/// Test C — Empty sentinel block: markers present, no rows between them → 0 appended, 0 skipped.
/// State last_scan_at STILL bumped (scan event regardless of empty block).
#[test]
fn empty_sentinel_block_zero_counts_state_still_bumps() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let report_path = tmp.path().join("report.md");
    let inbox_path = tmp.path().join("inbox.md");
    let state_path = tmp.path().join("state.json");

    // Craft minimal report with empty sentinel block.
    let empty_report = "# Agent Report\n\nNo advisories found.\n\n\
        <!-- INBOX_APPEND_START -->\n\
        <!-- INBOX_APPEND_END -->\n";
    std::fs::write(&report_path, empty_report).unwrap();
    std::fs::copy(fixtures_dir().join("inbox-baseline.md"), &inbox_path).unwrap();

    // Minimal state with 1 pre-existing ID.
    let pre_state_json = serde_json::json!({
        "schema_version": 1,
        "last_scan_at": "2026-05-23T12:00:00Z",
        "seen_advisories": ["CVE-2026-7777"],
        "agent_version": "advisory-watch@0.1.0"
    });
    std::fs::write(
        &state_path,
        serde_json::to_string_pretty(&pre_state_json).unwrap() + "\n",
    )
    .unwrap();

    let pre = read_state(&state_path);
    let pre_ids = seen_ids(&pre);
    let pre_last_scan = pre["last_scan_at"].as_str().unwrap().to_string();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "scan-and-append",
            "--report",
            report_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("\"appended\":0"))
        .stdout(contains("\"skipped_dedup\":0"));

    let post = read_state(&state_path);
    let post_ids = seen_ids(&post);

    // No new IDs added (empty block).
    assert_eq!(
        post_ids, pre_ids,
        "empty block: seen_advisories must be unchanged"
    );
    // last_scan_at STILL bumped (scan event).
    let post_last_scan = post["last_scan_at"].as_str().unwrap().to_string();
    assert!(
        post_last_scan > pre_last_scan,
        "empty block still counts as scan event: last_scan_at must be bumped"
    );
    // Inbox structurally unchanged (no rows inserted).
    let inbox_post = std::fs::read_to_string(&inbox_path).expect("read inbox post");
    assert!(
        inbox_post.contains("## Rows"),
        "## Rows heading must survive empty-block run"
    );
    // Existing rows still present.
    assert!(inbox_post.contains("CVE-OLD-1"));
}
