//! ref-tools CLI
//!
//! Reference verification tools with headless Chrome.
//! Bypasses bot protection (403/999) that blocks curl/wget.

use anyhow::Result;
use clap::{Parser, Subcommand};

mod browser;
mod check_links;
mod extract;
mod refresh_data;

use check_links::{CheckLinksArgs, run_check_links};
use refresh_data::{RefreshDataArgs, run_refresh_data};

#[derive(Parser)]
#[command(name = "ref-tools")]
#[command(author = "RoyalBit Inc.")]
#[command(version)]
#[command(about = "Reference verification tools with headless Chrome")]
#[command(long_about = "Bypasses bot protection (403/999) that blocks curl/wget.\n\nCommands:\n  check-links    Check URL health in markdown files\n  refresh-data   Extract live data from URLs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check URL health in markdown files or single URLs
    CheckLinks(CheckLinksArgs),
    /// Extract live data from URLs (market sizes, pricing, statistics)
    RefreshData(RefreshDataArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::CheckLinks(args) => run_check_links(args).await,
        Commands::RefreshData(args) => run_refresh_data(args).await,
    }
}
