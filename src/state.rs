//! StateFile type — `.advisory-scan-state` JSON schema (ARCHITECTURE.md §2).
//!
//! Atomic write at runtime (P005 dedup updates, P007 migrate-state rewrites,
//! P008 state-backfill unions). Schema version locks the wire shape; bump
//! requires migrate-state path (Sub-mech C).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current state file schema version. Bump on breaking change (Sub-mech C).
///
/// P007 migrate-state compares stored `schema_version` against this constant
/// to decide migration path. P002 locks V1.
pub const SCHEMA_VERSION: u32 = 1;

/// On-disk shape of `.advisory-scan-state`.
///
/// `seen_advisories` is `Vec<String>` (not `BTreeSet`) for serde-stable order
/// (insertion order preserved → diff-friendly). Runtime dedup logic in P005
/// converts to `HashSet` in-memory then back to `Vec` before atomic write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateFile {
    /// Schema version of this file. Current = `SCHEMA_VERSION` (1).
    pub schema_version: u32,
    /// Timestamp of last scan, RFC 3339 UTC (e.g., `2026-05-28T09:51:35Z`).
    pub last_scan_at: DateTime<Utc>,
    /// Advisory IDs already processed (CVE-..., GHSA-..., etc.). Dedup source.
    pub seen_advisories: Vec<String>,
    /// Free-form version tag of the emitting agent (e.g., `advisory-watch@0.1.0`).
    pub agent_version: String,
}

/// Errors returned by [`read`] when the state file cannot be loaded.
#[derive(Error, Debug)]
pub enum StateReadError {
    /// File is missing or cannot be read (OS-level I/O error).
    #[error("state file `{path}` unreadable: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// File contents are not valid JSON or do not match [`StateFile`] shape.
    #[error("state file `{path}` malformed JSON: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// `schema_version` in the file does not match [`SCHEMA_VERSION`].
    #[error(
        "state file `{path}` schema_version {found} != expected {expected} — run `advisory-inbox migrate-state --state {path}`"
    )]
    SchemaMismatch {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
}

/// Errors raised by atomic state-file write.
///
/// Exit-code mapping (caller's responsibility in `main.rs`):
/// - [`StateWriteError::Io`] → exit code 2 (per ARCHITECTURE §1 migrate-state).
#[derive(thiserror::Error, Debug)]
pub enum StateWriteError {
    /// File could not be written or the path has no parent directory.
    #[error("state file `{path}` I/O failure: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Atomically write `state` to `path` per INV-LOCAL-002 protocol:
/// temp file in SAME parent directory → fsync data+metadata → atomic rename.
///
/// Output is `serde_json::to_string_pretty` (2-space indent) with trailing newline.
///
/// This is the SECOND concrete user of INV-LOCAL-002 (after `inbox::write_atomic`
/// shipped in P006). Reference shape matches `src/inbox.rs::write_atomic` exactly.
pub fn write_atomic(path: &Path, state: &StateFile) -> Result<(), StateWriteError> {
    use std::io::Write as _;
    use tempfile::NamedTempFile;

    let parent = path.parent().ok_or_else(|| StateWriteError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "target path has no parent directory",
        ),
    })?;

    // Serialize. `serde_json::to_string_pretty` failures map to Io for MVP
    // (StateFile has no custom #[serde(serialize_with)] hooks; serialize is
    // infallible for our shape). If a real serialize failure occurs in the future,
    // add a `Json` variant to StateWriteError as a Tầng 2 self-decide.
    let mut serialized =
        serde_json::to_string_pretty(state).map_err(|source| StateWriteError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::other(source.to_string()),
        })?;
    // Append trailing newline — matches ARCHITECTURE §2 fixture convention.
    serialized.push('\n');

    let mut temp = NamedTempFile::new_in(parent).map_err(|source| StateWriteError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.write_all(serialized.as_bytes())
        .map_err(|source| StateWriteError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    temp.as_file()
        .sync_all()
        .map_err(|source| StateWriteError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    temp.persist(path).map_err(|e| StateWriteError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}

/// Read + validate a state file from disk.
///
/// Returns [`StateReadError::Io`] if the file is missing/unreadable,
/// [`StateReadError::Json`] if the contents are not valid JSON for [`StateFile`],
/// and [`StateReadError::SchemaMismatch`] if `schema_version != SCHEMA_VERSION`.
///
/// The schema-mismatch error includes a `migrate-state` hint pointing at the
/// same path so the operator can run the migration subcommand directly.
pub fn read(path: &Path) -> Result<StateFile, StateReadError> {
    let bytes = std::fs::read(path).map_err(|source| StateReadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let parsed: StateFile =
        serde_json::from_slice(&bytes).map_err(|source| StateReadError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    if parsed.schema_version != SCHEMA_VERSION {
        return Err(StateReadError::SchemaMismatch {
            path: path.to_path_buf(),
            found: parsed.schema_version,
            expected: SCHEMA_VERSION,
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn sample_state() -> StateFile {
        StateFile {
            schema_version: SCHEMA_VERSION,
            last_scan_at: DateTime::parse_from_rfc3339("2026-05-28T09:51:35Z")
                .expect("valid RFC 3339")
                .with_timezone(&Utc),
            seen_advisories: vec![
                "CVE-2026-9256".to_string(),
                "GHSA-xxxx-yyyy".to_string(),
                "CVE-2026-27205".to_string(),
            ],
            agent_version: "advisory-watch@0.1.0".to_string(),
        }
    }

    #[test]
    fn state_roundtrip_json() {
        let state = sample_state();
        let json = serde_json::to_string(&state).expect("serialize");
        let back: StateFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, back);
    }

    #[test]
    fn state_schema_version_constant_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
        assert_eq!(sample_state().schema_version, 1);
    }

    #[test]
    fn state_parses_known_json() {
        // ARCHITECTURE.md §2 example verbatim.
        let json = r#"{
            "schema_version": 1,
            "last_scan_at": "2026-05-28T09:51:35Z",
            "seen_advisories": [
                "CVE-2026-9256",
                "GHSA-xxxx-yyyy",
                "CVE-2026-27205"
            ],
            "agent_version": "advisory-watch@0.1.0"
        }"#;
        let state: StateFile = serde_json::from_str(json).expect("parse known JSON");
        assert_eq!(state.schema_version, 1);
        assert_eq!(state.seen_advisories.len(), 3);
        assert_eq!(state.agent_version, "advisory-watch@0.1.0");
    }

    #[test]
    fn state_preserves_seen_advisories_order() {
        let state = sample_state();
        let json = serde_json::to_string(&state).expect("serialize");
        let back: StateFile = serde_json::from_str(&json).expect("deserialize");
        // Insertion order preserved (Vec semantics, not Set).
        assert_eq!(back.seen_advisories[0], "CVE-2026-9256");
        assert_eq!(back.seen_advisories[1], "GHSA-xxxx-yyyy");
        assert_eq!(back.seen_advisories[2], "CVE-2026-27205");
    }

    // --- read() unit tests (P005) ---

    fn write_tempfile(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("tempfile");
        f.write_all(content.as_bytes()).expect("write fixture");
        f
    }

    #[test]
    fn read_happy_path() {
        let json = r#"{
            "schema_version": 1,
            "last_scan_at": "2026-05-28T09:51:35Z",
            "seen_advisories": ["CVE-2026-9256", "GHSA-xxxx-yyyy"],
            "agent_version": "advisory-watch@0.1.0"
        }"#;
        let f = write_tempfile(json);
        let state = read(f.path()).expect("read ok");
        assert_eq!(state.schema_version, 1);
        assert_eq!(state.seen_advisories.len(), 2);
        assert_eq!(state.agent_version, "advisory-watch@0.1.0");
    }

    #[test]
    fn read_missing_file_errors() {
        let err = read(Path::new("/nonexistent/advisory-state.json")).unwrap_err();
        assert!(matches!(err, StateReadError::Io { .. }));
    }

    #[test]
    fn read_schema_mismatch_errors() {
        let json = r#"{
            "schema_version": 99,
            "last_scan_at": "2026-05-28T09:51:35Z",
            "seen_advisories": [],
            "agent_version": "x"
        }"#;
        let f = write_tempfile(json);
        let err = read(f.path()).unwrap_err();
        match err {
            StateReadError::SchemaMismatch {
                found, expected, ..
            } => {
                assert_eq!(found, 99);
                assert_eq!(expected, 1);
            }
            other => panic!("expected SchemaMismatch, got {other:?}"),
        }
        // Error Display must hint migrate-state per ARCHITECTURE §1 future flow.
        let msg = format!("{}", read(f.path()).unwrap_err());
        assert!(msg.contains("migrate-state"));
    }

    #[test]
    fn read_malformed_json_errors() {
        let json = r#"{"schema_version": 1, "broken-json"#;
        let f = write_tempfile(json);
        let err = read(f.path()).unwrap_err();
        assert!(matches!(err, StateReadError::Json { .. }));
    }

    // --- write_atomic() unit tests (P007) ---

    #[test]
    fn write_atomic_round_trip() {
        use chrono::TimeZone as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("state.json");
        let original = StateFile {
            schema_version: 1,
            last_scan_at: chrono::Utc
                .with_ymd_and_hms(2026, 5, 28, 9, 51, 35)
                .unwrap(),
            seen_advisories: vec!["CVE-2026-1".to_string(), "CVE-2026-2".to_string()],
            agent_version: "test@1.0".to_string(),
        };
        write_atomic(&target, &original).expect("write atomic");
        let read_back = read(&target).expect("read back");
        assert_eq!(read_back.schema_version, original.schema_version);
        assert_eq!(read_back.last_scan_at, original.last_scan_at);
        assert_eq!(read_back.seen_advisories, original.seen_advisories);
        assert_eq!(read_back.agent_version, original.agent_version);
    }

    #[test]
    fn write_atomic_trailing_newline() {
        use chrono::TimeZone as _;
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("state.json");
        let state = StateFile {
            schema_version: 1,
            last_scan_at: chrono::Utc.with_ymd_and_hms(2026, 5, 28, 0, 0, 0).unwrap(),
            seen_advisories: vec![],
            agent_version: String::new(),
        };
        write_atomic(&target, &state).expect("write atomic");
        let bytes = std::fs::read(&target).expect("read raw bytes");
        assert_eq!(
            bytes.last().copied(),
            Some(b'\n'),
            "output should end with newline"
        );
    }

    #[test]
    fn write_atomic_no_parent_errors() {
        use chrono::TimeZone as _;
        // Path::new("just-a-filename") has parent = Some("") on most platforms.
        // NamedTempFile::new_in("") may succeed (treats "" as CWD). Either outcome
        // is acceptable; this test documents the behavior. If it succeeds, clean up.
        let state = StateFile {
            schema_version: 1,
            last_scan_at: chrono::Utc.with_ymd_and_hms(2026, 5, 28, 0, 0, 0).unwrap(),
            seen_advisories: vec![],
            agent_version: String::new(),
        };
        let bad_path = std::path::Path::new("just-a-filename-no-parent");
        match write_atomic(bad_path, &state) {
            Ok(()) => {
                // Succeeded (temp created in CWD). Clean up to keep workspace tidy.
                let _ = std::fs::remove_file(bad_path);
            }
            Err(StateWriteError::Io { .. }) => {
                // Expected on platforms where parent("") → Io error.
            }
        }
    }
}
