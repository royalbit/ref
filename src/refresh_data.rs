//! refresh-data command: Extract live data from URLs

use crate::browser::BrowserPool;
use crate::extract::{extract_amounts, extract_percentages, AmountMatch};
use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use std::io::{self, Write};
use tokio::fs;

#[derive(Args)]
pub struct RefreshDataArgs {
    /// Extract data from a single URL
    #[arg(long)]
    url: Option<String>,

    /// Markdown file to process (extract URLs and refresh data)
    #[arg(value_name = "FILE")]
    file: Option<String>,

    /// Filter by extractor type (instagram, statista, generic)
    #[arg(long, value_name = "TYPE")]
    filter: Option<String>,

    /// Timeout per URL in milliseconds
    #[arg(long, default_value = "20000")]
    timeout: u64,
}

/// Configuration for refresh-data
pub struct RefreshConfig {
    pub timeout_ms: u64,
}

/// Extracted data from a URL
#[derive(Debug, Serialize)]
pub struct ExtractedData {
    pub url: String,
    #[serde(rename = "type")]
    pub extractor_type: String,
    pub success: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amounts: Option<Vec<AmountMatch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentages: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub followers: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timestamp: String,
}

/// Report containing all extractions
#[derive(Debug, Serialize)]
pub struct RefreshReport {
    pub results: Vec<ExtractedData>,
    pub timestamp: String,
}

/// Run the refresh-data command
pub async fn run_refresh_data(args: RefreshDataArgs) -> Result<()> {
    let urls = get_extractable_urls(&args).await?;

    if urls.is_empty() {
        eprintln!("No extractable URLs found.");
        std::process::exit(1);
    }

    eprintln!("Found {} URLs to extract data from\n", urls.len());

    let config = RefreshConfig {
        timeout_ms: args.timeout,
    };

    let report = refresh_data(&urls, &config).await?;

    // Output JSON to stdout
    println!("{}", serde_json::to_string_pretty(&report)?);

    // Summary to stderr
    let success_count = report.results.iter().filter(|r| r.success).count();
    eprintln!("\n--- EXTRACTION SUMMARY ---");
    eprintln!("Success: {}/{}", success_count, report.results.len());

    // Show Instagram results
    let instagram: Vec<_> = report
        .results
        .iter()
        .filter(|r| r.extractor_type == "instagram" && r.success)
        .collect();

    if !instagram.is_empty() {
        eprintln!("\nInstagram Follower Counts:");
        for r in instagram {
            eprintln!(
                "  @{}: {}",
                r.username.as_deref().unwrap_or("unknown"),
                r.followers.as_deref().unwrap_or("N/A")
            );
        }
    }

    Ok(())
}

/// Determine extractor type from URL
fn get_extractor_type(url: &str) -> &'static str {
    if url.contains("instagram.com") {
        "instagram"
    } else if url.contains("statista.com") {
        "statista"
    } else {
        "generic"
    }
}

/// Get URLs to extract from
async fn get_extractable_urls(args: &RefreshDataArgs) -> Result<Vec<(String, String)>> {
    if let Some(url) = &args.url {
        let ext_type = get_extractor_type(url);
        return Ok(vec![(url.clone(), ext_type.to_string())]);
    }

    if let Some(file) = &args.file {
        let content = fs::read_to_string(file)
            .await
            .with_context(|| format!("Failed to read file: {}", file))?;

        let urls = extract_extractable_urls(&content);

        let filtered: Vec<_> = if let Some(filter) = &args.filter {
            urls.into_iter()
                .filter(|(_, t)| t == filter)
                .collect()
        } else {
            urls
        };

        return Ok(filtered);
    }

    eprintln!("Usage:");
    eprintln!("  ref-tools refresh-data --url <URL>      Extract from single URL");
    eprintln!("  ref-tools refresh-data <file.md>        Extract from all URLs in file");
    eprintln!("  ref-tools refresh-data <file.md> --filter instagram");
    std::process::exit(1);
}

/// Extract URLs that have extractors
fn extract_extractable_urls(content: &str) -> Vec<(String, String)> {
    use regex::Regex;
    use std::collections::HashSet;

    let patterns = [
        (Regex::new(r"https?://(?:www\.)?instagram\.com/[^\s\)\]]+").unwrap(), "instagram"),
        (Regex::new(r"https?://(?:www\.)?statista\.com/[^\s\)\]]+").unwrap(), "statista"),
        (Regex::new(r"https?://(?:www\.)?(?:influencermarketinghub|emarketer|techcrunch)\.com/[^\s\)\]]+").unwrap(), "market"),
    ];

    let mut seen = HashSet::new();
    let mut urls = Vec::new();

    for (re, ext_type) in &patterns {
        for mat in re.find_iter(content) {
            let url = mat.as_str().trim_end_matches(|c| matches!(c, ',' | '.' | ')' | ']'));
            if !seen.contains(url) {
                seen.insert(url.to_string());
                urls.push((url.to_string(), ext_type.to_string()));
            }
        }
    }

    urls
}

/// Extract data from multiple URLs
pub async fn refresh_data(urls: &[(String, String)], config: &RefreshConfig) -> Result<RefreshReport> {
    let pool = BrowserPool::new(1).await?; // Sequential for rate limiting
    let mut results = Vec::with_capacity(urls.len());

    for (url, ext_type) in urls {
        eprint!("Extracting [{}]: {}...", ext_type, truncate(url, 50));
        io::stderr().flush().ok();

        let page = pool.new_page().await?;
        let result = extract_from_page(&page, url, ext_type, config.timeout_ms).await;

        eprintln!(" {}", if result.success { "OK" } else { "FAIL" });
        results.push(result);

        // Rate limit
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    pool.close().await?;

    Ok(RefreshReport {
        results,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Extract data from a single page
async fn extract_from_page(
    page: &crate::browser::BrowserPage,
    url: &str,
    ext_type: &str,
    timeout_ms: u64,
) -> ExtractedData {
    let timestamp = chrono::Utc::now().to_rfc3339();

    let nav_result = page.goto(url, timeout_ms).await;
    if let Err(e) = nav_result {
        return ExtractedData {
            url: url.to_string(),
            extractor_type: ext_type.to_string(),
            success: false,
            title: None,
            description: None,
            amounts: None,
            percentages: None,
            followers: None,
            username: None,
            error: Some(e.to_string()),
            timestamp,
        };
    }

    let content = match page.content().await {
        Ok(c) => c,
        Err(e) => {
            return ExtractedData {
                url: url.to_string(),
                extractor_type: ext_type.to_string(),
                success: false,
                title: None,
                description: None,
                amounts: None,
                percentages: None,
                followers: None,
                username: None,
                error: Some(e.to_string()),
                timestamp,
            };
        }
    };

    match ext_type {
        "instagram" => extract_instagram(url, &content, timestamp),
        "statista" => extract_statista(url, &content, timestamp),
        _ => extract_generic(url, &content, timestamp),
    }
}

fn extract_instagram(url: &str, content: &str, timestamp: String) -> ExtractedData {
    use regex::Regex;

    let follower_re = Regex::new(r"([0-9,.]+[KMB]?)\s*[Ff]ollowers").unwrap();
    let followers = follower_re
        .captures(content)
        .map(|c| c[1].to_string());

    // Extract username from URL
    let username = url
        .trim_end_matches('/')
        .split('/')
        .last()
        .map(|s| s.to_string());

    ExtractedData {
        url: url.to_string(),
        extractor_type: "instagram".to_string(),
        success: true,
        title: None,
        description: None,
        amounts: None,
        percentages: None,
        followers,
        username,
        error: None,
        timestamp,
    }
}

fn extract_statista(url: &str, content: &str, timestamp: String) -> ExtractedData {
    let amounts = extract_amounts(content);
    let percentages = extract_percentages(content);

    // Try to extract title
    let title = extract_title(content);

    ExtractedData {
        url: url.to_string(),
        extractor_type: "statista".to_string(),
        success: true,
        title,
        description: None,
        amounts: Some(amounts),
        percentages: Some(percentages),
        followers: None,
        username: None,
        error: None,
        timestamp,
    }
}

fn extract_generic(url: &str, content: &str, timestamp: String) -> ExtractedData {
    let amounts = extract_amounts(content);
    let percentages = extract_percentages(content);
    let title = extract_title(content);
    let description = extract_description(content);

    ExtractedData {
        url: url.to_string(),
        extractor_type: "generic".to_string(),
        success: true,
        title,
        description,
        amounts: if amounts.is_empty() { None } else { Some(amounts) },
        percentages: if percentages.is_empty() { None } else { Some(percentages) },
        followers: None,
        username: None,
        error: None,
        timestamp,
    }
}

fn extract_title(content: &str) -> Option<String> {
    use regex::Regex;

    // Try <h1> first
    let h1_re = Regex::new(r"<h1[^>]*>([^<]+)</h1>").unwrap();
    if let Some(cap) = h1_re.captures(content) {
        return Some(cap[1].trim().to_string());
    }

    // Fall back to <title>
    let title_re = Regex::new(r"<title[^>]*>([^<]+)</title>").unwrap();
    title_re.captures(content).map(|c| c[1].trim().to_string())
}

fn extract_description(content: &str) -> Option<String> {
    use regex::Regex;

    let desc_re = Regex::new(r#"<meta[^>]*name=["']description["'][^>]*content=["']([^"']+)["']"#).unwrap();
    desc_re.captures(content).map(|c| c[1].to_string())
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
    fn test_get_extractor_type() {
        assert_eq!(get_extractor_type("https://instagram.com/user"), "instagram");
        assert_eq!(get_extractor_type("https://www.statista.com/stats"), "statista");
        assert_eq!(get_extractor_type("https://example.com"), "generic");
    }

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>Test Page</title></head><body><h1>Main Title</h1></body></html>";
        assert_eq!(extract_title(html), Some("Main Title".to_string()));

        let html_no_h1 = "<html><head><title>Test Page</title></head></html>";
        assert_eq!(extract_title(html_no_h1), Some("Test Page".to_string()));
    }

    #[test]
    fn test_extract_instagram() {
        let content = "Profile has 577K Followers and 100 posts";
        let result = extract_instagram("https://instagram.com/testuser", content, "2025-01-01T00:00:00Z".to_string());
        assert_eq!(result.followers, Some("577K".to_string()));
        assert_eq!(result.username, Some("testuser".to_string()));
    }

    #[test]
    fn test_extract_extractable_urls() {
        let content = r#"
            Check https://instagram.com/user1 and
            https://www.statista.com/statistics/123
            and https://example.com for more.
        "#;

        let urls = extract_extractable_urls(content);
        assert_eq!(urls.len(), 2); // Only instagram and statista
        assert!(urls.iter().any(|(u, _)| u.contains("instagram")));
        assert!(urls.iter().any(|(u, _)| u.contains("statista")));
    }
}
