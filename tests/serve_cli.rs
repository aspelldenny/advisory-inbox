//! Integration tests for `advisory-inbox serve` (MCP handshake — P010).
//!
//! These tests spawn the binary, write JSON-RPC `initialize` to stdin, read response
//! from stdout, and assert MCP handshake shape per spec.
//!
//! P010 ships handshake only — `tools/list` tests deferred to P011.

use assert_cmd::cargo::CommandCargoExt;
use std::io::Write;
use std::process::{Command, Stdio};

/// MCP `initialize` JSON-RPC request payload.
/// Uses protocol version 2024-11-05 to verify rmcp backward-compat negotiation
/// (rmcp 1.7.0 default/LATEST is 2025-11-25 but accepts older client versions).
const INIT_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"0.0.0"}}}"#;

#[test]
fn serve_responds_to_initialize_with_server_info() {
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
        writeln!(stdin, "{}", INIT_REQUEST).expect("write initialize request");
    }
    // Close stdin to signal EOF — rmcp resolves .waiting() and child exits gracefully.
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for child");

    assert!(
        output.status.success(),
        "advisory-inbox serve did not exit 0 — status: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");

    // Assert MCP initialize response shape: serverInfo.name + id echo + jsonrpc envelope.
    assert!(
        stdout.contains("\"name\":\"advisory-inbox\""),
        "stdout missing serverInfo.name=\"advisory-inbox\": {}",
        stdout
    );
    assert!(
        stdout.contains("\"id\":1"),
        "stdout missing JSON-RPC id=1 echo: {}",
        stdout
    );
    assert!(
        stdout.contains("\"jsonrpc\":\"2.0\""),
        "stdout missing JSON-RPC envelope marker: {}",
        stdout
    );
}
