//! check-links command: Check URL health in markdown files

use crate::browser::BrowserPool;
use crate::extract::extract_urls;
use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use tokio::fs;

#[derive(Args)]
pub struct CheckLinksArgs {
    /// Markdown file to check URLs from
    #[arg(value_name = "FILE")]
    file: Option<String>,

    /// Check a single URL
    #[arg(long)]
    url: Option<String>,

    /// Read URLs from stdin (one per line)
    #[arg(long)]
    stdin: bool,

    /// Number of parallel browser tabs (1-20)
    #[arg(short, long, default_value = "5", value_parser = clap::value_parser!(u8).range(1..=20))]
    concurrency: u8,

    /// Timeout per URL in milliseconds
    #[arg(long, default_value = "15000")]
    timeout: u64,

    /// Number of retries on failure
    #[arg(long, default_value = "1")]
    retries: u8,
}

/// Configuration for check-links
pub struct CheckLinksConfig {
    pub concurrency: usize,
    pub timeout_ms: u64,
    pub retries: u8,
}

/// Result for a single link check
#[derive(Debug, Serialize)]
pub struct LinkResult {
    pub url: String,
    pub status: u16,
    #[serde(rename = "statusText")]
    pub status_text: String,
    pub title: Option<String>,
    pub error: Option<String>,
    pub time: u64,
}

/// Summary statistics
#[derive(Debug, Serialize)]
pub struct Summary {
    pub total: usize,
    pub ok: usize,
    pub redirects: usize,
    #[serde(rename = "clientErrors")]
    pub client_errors: usize,
    #[serde(rename = "serverErrors")]
    pub server_errors: usize,
    pub blocked: usize,
    pub failed: usize,
}

/// Full report
#[derive(Debug, Serialize)]
pub struct LinkReport {
    pub summary: Summary,
    #[serde(rename = "byStatus")]
    pub by_status: HashMap<u16, usize>,
    pub results: Vec<LinkResult>,
    pub timestamp: String,
}

/// Run the check-links command
pub async fn run_check_links(args: CheckLinksArgs) -> Result<()> {
    let urls = get_urls(&args).await?;

    if urls.is_empty() {
        eprintln!("No URLs found.");
        std::process::exit(1);
    }

    eprintln!("Found {} URLs to check (concurrency: {})\n", urls.len(), args.concurrency);

    let config = CheckLinksConfig {
        concurrency: args.concurrency as usize,
        timeout_ms: args.timeout,
        retries: args.retries,
    };

    let report = check_links(&urls, &config).await?;

    // Output JSON to stdout
    println!("{}", serde_json::to_string_pretty(&report)?);

    // Summary to stderr
    eprintln!("\n--- SUMMARY ---");
    eprintln!("Total:    {}", report.summary.total);
    eprintln!("OK (2xx): {}", report.summary.ok);
    eprintln!("Blocked:  {}", report.summary.blocked);
    eprintln!("Errors:   {}", report.summary.client_errors + report.summary.server_errors);
    eprintln!("Failed:   {}", report.summary.failed);

    Ok(())
}

/// Get URLs from file, --url, or stdin
async fn get_urls(args: &CheckLinksArgs) -> Result<Vec<String>> {
    if let Some(url) = &args.url {
        return Ok(vec![url.clone()]);
    }

    if args.stdin {
        let stdin = io::stdin();
        let urls: Vec<String> = stdin
            .lock()
            .lines()
            .filter_map(|line| line.ok())
            .filter(|line| line.starts_with("http"))
            .collect();
        return Ok(urls);
    }

    if let Some(file) = &args.file {
        let content = fs::read_to_string(file)
            .await
            .with_context(|| format!("Failed to read file: {}", file))?;
        return Ok(extract_urls(&content));
    }

    eprintln!("Usage:");
    eprintln!("  ref-tools check-links <file.md>    Check URLs in markdown file");
    eprintln!("  ref-tools check-links --url <URL>  Check single URL");
    eprintln!("  ref-tools check-links --stdin      Read URLs from stdin");
    std::process::exit(1);
}

/// Check multiple links and generate report
pub async fn check_links(urls: &[String], config: &CheckLinksConfig) -> Result<LinkReport> {
    let pool = BrowserPool::new(config.concurrency).await?;
    let mut results = Vec::with_capacity(urls.len());

    for url in urls {
        eprint!("Checking: {}...", truncate(url, 60));
        io::stderr().flush().ok();

        let page = pool.new_page().await?;
        let mut result = page.goto(url, config.timeout_ms).await?;

        // Retry on failure
        if result.status == 0 && config.retries > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            result = page.goto(url, config.timeout_ms).await?;
        }

        eprintln!(" {}", result.status);

        results.push(LinkResult {
            url: url.clone(),
            status: result.status,
            status_text: result.status_text,
            title: result.title,
            error: result.error,
            time: result.time_ms,
        });
    }

    pool.close().await?;

    Ok(generate_report(results))
}

fn generate_report(mut results: Vec<LinkResult>) -> LinkReport {
    let mut summary = Summary {
        total: results.len(),
        ok: 0,
        redirects: 0,
        client_errors: 0,
        server_errors: 0,
        blocked: 0,
        failed: 0,
    };

    let mut by_status: HashMap<u16, usize> = HashMap::new();

    for r in &results {
        *by_status.entry(r.status).or_insert(0) += 1;

        match r.status {
            200..=299 => summary.ok += 1,
            300..=399 => summary.redirects += 1,
            403 => summary.blocked += 1,
            400..=499 => summary.client_errors += 1,
            500..=599 => summary.server_errors += 1,
            _ => summary.failed += 1,
        }
    }

    // Sort by status (errors first)
    results.sort_by_key(|r| if r.status == 0 { 999 } else { r.status });

    LinkReport {
        summary,
        by_status,
        results,
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a very long string", 10), "this is...");
    }

    #[test]
    fn test_generate_report() {
        let results = vec![
            LinkResult {
                url: "https://ok.com".into(),
                status: 200,
                status_text: "OK".into(),
                title: Some("OK".into()),
                error: None,
                time: 100,
            },
            LinkResult {
                url: "https://notfound.com".into(),
                status: 404,
                status_text: "Not Found".into(),
                title: None,
                error: None,
                time: 50,
            },
            LinkResult {
                url: "https://blocked.com".into(),
                status: 403,
                status_text: "Forbidden".into(),
                title: None,
                error: None,
                time: 75,
            },
        ];

        let report = generate_report(results);
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.ok, 1);
        assert_eq!(report.summary.blocked, 1);
        assert_eq!(report.summary.client_errors, 1);
    }
}
