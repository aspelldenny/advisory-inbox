//! Stub for `state-backfill` subcommand. Real logic wired in P008.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(state: PathBuf, inbox: PathBuf, dry_run: bool) -> Result<()> {
    println!(
        "TODO: state-backfill (state={:?}, inbox={:?}, dry_run={}) — wired in P008",
        state, inbox, dry_run
    );
    Ok(())
}
