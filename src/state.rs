//! StateFile type — `.advisory-scan-state` JSON schema (ARCHITECTURE.md §2).
//!
//! Atomic write at runtime (P005 dedup updates, P007 migrate-state rewrites,
//! P008 state-backfill unions). Schema version locks the wire shape; bump
//! requires migrate-state path (Sub-mech C).

// Types are scaffold-declared for P005+ consumers. Allow dead_code until
// subcmd wire-in phiếu (P005+) imports them.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
