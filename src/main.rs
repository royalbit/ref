//! ref-tools CLI
//!
//! Reference verification tools with headless Chrome.
//! Bypasses bot protection (403/999) that blocks curl/wget.

use anyhow::Result;
use clap::{Parser, Subcommand};

mod browser;
mod check_links;
mod extract;
mod fetch;
mod refresh_data;
mod schema;
mod verify_refs;

use check_links::{run_check_links, CheckLinksArgs};
use fetch::{run_fetch, FetchArgs};
use refresh_data::{run_refresh_data, RefreshDataArgs};
use verify_refs::{run_verify_refs, VerifyRefsArgs};

#[derive(Parser)]
#[command(name = "ref-tools")]
#[command(author = "RoyalBit Inc.")]
#[command(version)]
#[command(about = "Reference verification tools with headless Chrome")]
#[command(
    long_about = "Bypasses bot protection (403/999) that blocks curl/wget.\n\nCommands:\n  fetch          Fetch URL, convert HTML to JSON/YAML (for LLMs)\n  check-links    Check URL health in markdown files\n  refresh-data   Extract live data from URLs"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch URL and convert HTML to structured JSON/YAML (optimized for LLMs)
    Fetch(FetchArgs),
    /// Check URL health in markdown files or single URLs
    CheckLinks(CheckLinksArgs),
    /// Extract live data from URLs (market sizes, pricing, statistics)
    RefreshData(RefreshDataArgs),
    /// Verify references.yaml entries and update status
    VerifyRefs(VerifyRefsArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Fetch(args) => run_fetch(args).await,
        Commands::CheckLinks(args) => run_check_links(args).await,
        Commands::RefreshData(args) => run_refresh_data(args).await,
        Commands::VerifyRefs(args) => run_verify_refs(args).await,
    }
}
