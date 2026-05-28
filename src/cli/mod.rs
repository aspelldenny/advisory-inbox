//! CLI subcommand module registry.
//!
//! Each submodule exposes a `run` function called from `main.rs` after clap
//! dispatch. Logic bodies are stubs in P001; phiếu sau wire in real handlers
//! (P004 parse-report, P005 dedup, P006 append, etc.).

pub mod append;
pub mod dedup;
pub mod init;
pub mod migrate_state;
pub mod parse_report;
pub mod scan_and_append;
pub mod serve;
pub mod state_backfill;
