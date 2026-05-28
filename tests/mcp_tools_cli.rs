//! Integration tests for MCP tool dispatch (P011) — `advisory-inbox serve`.
//!
//! Verifies Sub-mech A (trigger): `tools/list` returns 6 named tools, and
//! `tools/call parse_report` round-trips a JSON-RPC response with the expected shape.
//!
//! Pattern: spawn binary in `serve` mode, write JSON-RPC messages to stdin
//! (one per line), close stdin to signal EOF, read all stdout lines, parse each
//! as JSON, find the response matching the request id.

use assert_cmd::cargo::CommandCargoExt;
use std::io::Write;
use std::process::{Command, Stdio};

// ──────────────────────────────────────────────────────────────
// JSON-RPC helpers
// ──────────────────────────────────────────────────────────────

/// MCP `initialize` JSON-RPC request (id=1).
const INIT_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"0.0.0"}}}"#;

/// MCP `tools/list` JSON-RPC request (id=2).
const TOOLS_LIST_REQUEST: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;

/// Agent report fixture with 2 rows between sentinel markers.
const REPORT_FIXTURE: &str = "# Advisory Report\n\
    <!-- INBOX_APPEND_START -->\n\
    | 2026-05-28 | CVE-2026-TEST1 | https://example.com/cve1 | crate-a@1.0 | src/lib.rs:10 | High | open | - |\n\
    | 2026-05-28 | CVE-2026-TEST2 | https://example.com/cve2 | crate-b@2.0 | src/main.rs:5 | Medium | open | - |\n\
    <!-- INBOX_APPEND_END -->\n";

/// Spawn `advisory-inbox serve`, write all `messages` (newline-separated) to stdin,
/// close stdin, collect stdout. Returns all stdout lines parsed as JSON.
fn run_mcp_session(messages: &[&str]) -> Vec<serde_json::Value> {
    let mut child = Command::cargo_bin("advisory-inbox")
        .expect("cargo_bin advisory-inbox")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn advisory-inbox serve");

    {
        let stdin = child.stdin.as_mut().expect("stdin handle");
        for msg in messages {
            writeln!(stdin, "{}", msg).expect("write MCP message");
        }
    }
    // Close stdin — signals EOF; rmcp resolves .waiting() and process exits.
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for child");
    assert!(
        output.status.success(),
        "advisory-inbox serve exited non-zero — status: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect()
}

/// Find the JSON-RPC response with the given numeric `id` from a list of parsed lines.
fn find_response(lines: &[serde_json::Value], id: u64) -> Option<&serde_json::Value> {
    lines
        .iter()
        .find(|v| v.get("id").and_then(|i| i.as_u64()) == Some(id))
}

// ──────────────────────────────────────────────────────────────
// Test: tools/list returns 6 tool names (Sub-mech A trigger)
// ──────────────────────────────────────────────────────────────

#[test]
fn tools_list_returns_six_tools() {
    let lines = run_mcp_session(&[INIT_REQUEST, TOOLS_LIST_REQUEST]);

    let response = find_response(&lines, 2).expect("no tools/list response found in stdout");

    let tools = response
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .expect("result.tools array missing");

    assert_eq!(
        tools.len(),
        6,
        "expected 6 tools, got {}. Response: {}",
        tools.len(),
        response
    );

    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();

    let expected: std::collections::HashSet<&str> = [
        "parse_report",
        "dedup",
        "append",
        "migrate_state",
        "state_backfill",
        "scan_and_append",
    ]
    .iter()
    .copied()
    .collect();

    let actual: std::collections::HashSet<&str> = names.iter().copied().collect();
    assert_eq!(actual, expected, "tool names mismatch — got: {:?}", names);
}

// ──────────────────────────────────────────────────────────────
// Test: tools/call parse_report round-trip (Sub-mech A trigger)
// ──────────────────────────────────────────────────────────────

#[test]
fn tools_call_parse_report_round_trip() {
    // Build the tools/call request with the fixture report as argument.
    let report_escaped = REPORT_FIXTURE.replace('\n', "\\n").replace('"', "\\\"");
    let call_request = format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"parse_report","arguments":{{"report_text":"{report_escaped}"}}}}}}"#
    );

    let lines = run_mcp_session(&[INIT_REQUEST, &call_request]);

    let response = find_response(&lines, 3).expect("no tools/call response found in stdout");

    // Response should have result.content array (rmcp CallToolResult shape).
    let content = response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .expect("result.content array missing in tools/call response");

    assert!(
        !content.is_empty(),
        "result.content is empty — expected at least one text block"
    );

    // Extract the text field from the first content block.
    let text = content[0]
        .get("text")
        .and_then(|t| t.as_str())
        .expect("content[0].text missing");

    // Parse the text as JSON to verify ParseReportOutput shape.
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("content[0].text is not valid JSON");

    assert_eq!(
        parsed["advisories_found"].as_u64(),
        Some(2),
        "expected 2 advisories_found"
    );

    let rows = parsed["rows"].as_array().expect("rows array missing");
    assert_eq!(rows.len(), 2, "expected 2 rows");

    // Verify first row has expected advisory_id.
    assert_eq!(
        rows[0]["advisory_id"].as_str(),
        Some("CVE-2026-TEST1"),
        "first row advisory_id mismatch"
    );
}
