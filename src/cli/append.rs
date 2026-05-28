//! Stub for `append` subcommand. Real logic wired in P006.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(inbox: PathBuf, rows_json: PathBuf) -> Result<()> {
    println!(
        "TODO: append (inbox={:?}, rows_json={:?}) — wired in P006",
        inbox, rows_json
    );
    Ok(())
}
