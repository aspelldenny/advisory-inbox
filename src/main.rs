//! advisory-inbox — CLI + MCP dual-mode binary.
//! Entry point: clap parse → dispatch to `cli::*` subcommand handler.
//!
//! See `docs/ARCHITECTURE.md` §1 for CLI surface, §5 for module layout.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cli;
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
        Commands::ParseReport { input } => cli::parse_report::run(input),
        Commands::Dedup { state, rows_json } => cli::dedup::run(state, rows_json),
        Commands::Append { inbox, rows_json } => cli::append::run(inbox, rows_json),
        Commands::MigrateState { state, dry_run } => cli::migrate_state::run(state, dry_run),
        Commands::StateBackfill {
            state,
            inbox,
            dry_run,
        } => cli::state_backfill::run(state, inbox, dry_run),
        Commands::ScanAndAppend {
            report,
            inbox,
            state,
        } => cli::scan_and_append::run(report, inbox, state),
        Commands::Serve => cli::serve::run(),
        Commands::Init {
            inbox_path,
            state_path,
        } => cli::init::run(inbox_path, state_path),
    }
}
