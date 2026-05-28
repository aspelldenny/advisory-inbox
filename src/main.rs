//! advisory-inbox — CLI + MCP dual-mode binary.
//! Entry point: clap parse → dispatch to `cli::*` subcommand handler.
//!
//! See `docs/ARCHITECTURE.md` §1 for CLI surface, §5 for module layout.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cli;
mod inbox;
mod row;
mod sentinel;
mod state;

#[derive(Parser)]
#[command(name = "advisory-inbox", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse sentinel block from agent report (stdin or --input <FILE>)
    ParseReport {
        /// Path to agent report markdown. Defaults to stdin.
        #[arg(long)]
        input: Option<PathBuf>,
    },
    /// Filter rows against state seen_advisories[]
    Dedup {
        /// Path to state JSON file
        #[arg(long)]
        state: PathBuf,
        /// Path to rows JSON (output of parse-report)
        #[arg(long = "rows-json")]
        rows_json: PathBuf,
    },
    /// Insert rows after `## Rows` heading in inbox markdown
    Append {
        /// Path to inbox markdown
        #[arg(long)]
        inbox: PathBuf,
        /// Path to rows JSON
        #[arg(long = "rows-json")]
        rows_json: PathBuf,
    },
    /// Convert legacy single-line ISO state file to JSON schema
    MigrateState {
        /// Path to state file
        #[arg(long)]
        state: PathBuf,
        /// Detect + report but do not write
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    /// Extract advisory IDs from inbox rows into state seen_advisories[]
    StateBackfill {
        /// Path to state JSON
        #[arg(long)]
        state: PathBuf,
        /// Path to inbox markdown
        #[arg(long)]
        inbox: PathBuf,
        /// Compute + report but do not write
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    /// Composite: parse → dedup → append + state update
    ScanAndAppend {
        /// Path to agent report. Omit for stdin.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Path to inbox markdown
        #[arg(long)]
        inbox: PathBuf,
        /// Path to state JSON
        #[arg(long)]
        state: PathBuf,
    },
    /// Start MCP server on stdin/stdout (JSON-RPC 2.0)
    Serve,
    /// Generate default config templates
    Init {
        /// Where to create the inbox markdown
        #[arg(long = "inbox-path")]
        inbox_path: Option<PathBuf>,
        /// Where to create the state JSON
        #[arg(long = "state-path")]
        state_path: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ParseReport { input } => {
            if let Err(e) = cli::parse_report::run(input) {
                // SentinelError (missing markers) → exit 1.
                // RowParseError, IO errors, and all other failures → exit 2.
                let code = if e.is::<crate::sentinel::SentinelError>() {
                    1
                } else {
                    2
                };
                eprintln!("error: {:#}", e);
                std::process::exit(code);
            }
            Ok(())
        }
        Commands::Dedup { state, rows_json } => {
            if let Err(e) = cli::dedup::run(state, rows_json) {
                let code = if e.is::<crate::state::StateReadError>() {
                    1
                } else {
                    2
                };
                eprintln!("error: {:#}", e);
                std::process::exit(code);
            }
            Ok(())
        }
        Commands::Append { inbox, rows_json } => {
            if let Err(e) = cli::append::run(inbox, rows_json) {
                let code = if let Some(ie) = e.downcast_ref::<crate::inbox::InboxError>() {
                    match ie {
                        crate::inbox::InboxError::MissingRowsHeading { .. } => 1,
                        crate::inbox::InboxError::ParseRow { .. } => 1,
                        crate::inbox::InboxError::Io { .. } => 2,
                    }
                } else {
                    2 // rows JSON malformed / unreadable / other serde err → exit 2
                };
                eprintln!("error: {:#}", e);
                std::process::exit(code);
            }
            Ok(())
        }
        Commands::MigrateState { state, dry_run } => {
            if let Err(e) = cli::migrate_state::run(state, dry_run) {
                let code = if e
                    .downcast_ref::<cli::migrate_state::MigrateError>()
                    .is_some()
                {
                    // FormatUnknown or UnsupportedSchema → exit 1.
                    1
                } else if e.downcast_ref::<crate::state::StateWriteError>().is_some() {
                    // Io write failure → exit 2.
                    2
                } else {
                    // Unexpected error category → exit 2 (write/IO bucket).
                    2
                };
                eprintln!("error: {:#}", e);
                std::process::exit(code);
            }
            Ok(())
        }
        Commands::StateBackfill {
            state,
            inbox,
            dry_run,
        } => {
            if let Err(e) = cli::state_backfill::run(state, inbox, dry_run) {
                let code = if e.downcast_ref::<crate::inbox::InboxError>().is_some()
                    || e.downcast_ref::<crate::state::StateReadError>().is_some()
                {
                    1 // input file invalid (inbox unparseable or state unreadable)
                } else {
                    2 // write failure or unexpected error → exit 2
                };
                eprintln!("error: {:#}", e);
                std::process::exit(code);
            }
            Ok(())
        }
        Commands::ScanAndAppend {
            report,
            inbox,
            state,
        } => {
            if let Err(e) = cli::scan_and_append::run(report, inbox, state) {
                let code = if e.downcast_ref::<crate::sentinel::SentinelError>().is_some()
                    || e.downcast_ref::<crate::state::StateReadError>().is_some()
                {
                    1 // sentinel missing or state unreadable → exit 1
                } else if e.downcast_ref::<crate::row::RowParseError>().is_some() {
                    2 // row parse failure → exit 2
                } else if let Some(ie) = e.downcast_ref::<crate::inbox::InboxError>() {
                    match ie {
                        crate::inbox::InboxError::MissingRowsHeading { .. } => 1,
                        crate::inbox::InboxError::ParseRow { .. } => 1,
                        crate::inbox::InboxError::Io { .. } => 2,
                    }
                } else if e.downcast_ref::<crate::state::StateWriteError>().is_some() {
                    2 // state write failure → exit 2
                } else {
                    // Fallback: stdin read fail / unexpected → exit 2 (write/IO bucket).
                    2
                };
                eprintln!("error: {:#}", e);
                std::process::exit(code);
            }
            Ok(())
        }
        Commands::Serve => cli::serve::run(),
        Commands::Init {
            inbox_path,
            state_path,
        } => cli::init::run(inbox_path, state_path),
    }
}
