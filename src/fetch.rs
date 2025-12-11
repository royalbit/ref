//! fetch command: Fetch URLs and convert HTML to structured JSON/YAML
//!
//! Optimized for LLM consumption - minimal tokens, maximum signal.
//! Supports parallel fetching of multiple URLs.

use crate::browser::BrowserPool;
use anyhow::Result;
use clap::Args;
use futures::future::join_all;
use scraper::{Html, Selector};
use serde::Serialize;
use std::sync::Arc;

#[derive(Args)]
pub struct FetchArgs {
    /// URLs to fetch (multiple allowed)
    #[arg(required = true)]
    pub urls: Vec<String>,

    /// Output format: json (default) or yaml
    #[arg(long, short, default_value = "json")]
    pub format: String,

    /// Parallel fetches (browser tabs)
    #[arg(long, short, default_value = "4")]
    pub parallel: usize,

    /// Timeout per URL in milliseconds
    #[arg(long, default_value = "30000")]
    pub timeout: u64,

    /// Max content length per page (0 = unlimited)
    #[arg(long, default_value = "50000")]
    pub max_chars: usize,
}

/// Minimal structured output for LLM consumption
#[derive(Debug, Serialize, Clone)]
pub struct Page {
    pub url: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub h: Vec<String>,
    pub len: usize,
}

/// Results wrapper for multiple pages
#[derive(Debug, Serialize)]
pub struct FetchResults {
    pub pages: Vec<Page>,
    pub total: usize,
    pub ok: usize,
    pub failed: usize,
}

pub async fn run_fetch(args: FetchArgs) -> Result<()> {
    let url_count = args.urls.len();
    let parallel = args.parallel.min(url_count).max(1);

    eprintln!(
        "Fetching {} URL{} ({} parallel)...",
        url_count,
        if url_count == 1 { "" } else { "s" },
        parallel
    );

    let pool = Arc::new(BrowserPool::new(parallel).await?);
    let timeout = args.timeout;
    let max_chars = args.max_chars;

    // Spawn parallel fetch tasks
    let tasks: Vec<_> = args
        .urls
        .into_iter()
        .map(|url| {
            let pool = Arc::clone(&pool);
            tokio::spawn(async move { fetch_one(&pool, &url, timeout, max_chars).await })
        })
        .collect();

    // Await all tasks
    let results: Vec<Page> = join_all(tasks)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();

    // Close browser
    if let Ok(pool) = Arc::try_unwrap(pool) {
        pool.close().await?;
    }

    let ok_count = results.iter().filter(|p| p.ok).count();
    let failed_count = results.len() - ok_count;

    // Output based on count
    let output = if results.len() == 1 {
        // Single URL: output just the page object
        match args.format.as_str() {
            "yaml" | "yml" => serde_yaml::to_string(&results[0])?,
            _ => serde_json::to_string_pretty(&results[0])?,
        }
    } else {
        // Multiple URLs: wrap in results object
        let fetch_results = FetchResults {
            pages: results,
            total: url_count,
            ok: ok_count,
            failed: failed_count,
        };
        match args.format.as_str() {
            "yaml" | "yml" => serde_yaml::to_string(&fetch_results)?,
            _ => serde_json::to_string_pretty(&fetch_results)?,
        }
    };

    println!("{}", output);
    eprintln!("Done: {}/{} OK", ok_count, url_count);

    Ok(())
}

async fn fetch_one(pool: &BrowserPool, url: &str, timeout: u64, max_chars: usize) -> Page {
    eprintln!("  -> {}", truncate(url, 60));

    let page = match pool.new_page().await {
        Ok(p) => p,
        Err(e) => {
            return Page {
                url: url.to_string(),
                ok: false,
                err: Some(e.to_string()),
                title: None,
                desc: None,
                text: None,
                h: vec![],
                len: 0,
            }
        }
    };

    let nav = match page.goto(url, timeout).await {
        Ok(n) => n,
        Err(e) => {
            return Page {
                url: url.to_string(),
                ok: false,
                err: Some(e.to_string()),
                title: None,
                desc: None,
                text: None,
                h: vec![],
                len: 0,
            }
        }
    };

    if nav.error.is_some() {
        return Page {
            url: url.to_string(),
            ok: false,
            err: nav.error,
            title: nav.title,
            desc: None,
            text: None,
            h: vec![],
            len: 0,
        };
    }

    match page.content().await {
        Ok(html) => parse(&html, url, max_chars),
        Err(e) => Page {
            url: url.to_string(),
            ok: false,
            err: Some(e.to_string()),
            title: None,
            desc: None,
            text: None,
            h: vec![],
            len: 0,
        },
    }
}

fn parse(html: &str, url: &str, max_chars: usize) -> Page {
    let doc = Html::parse_document(html);

    let title = select_text(&doc, "title")
        .or_else(|| select_attr(&doc, "meta[property='og:title']", "content"))
        .or_else(|| select_text(&doc, "h1"));

    let desc = select_attr(&doc, "meta[name='description']", "content")
        .or_else(|| select_attr(&doc, "meta[property='og:description']", "content"));

    let headings = extract_headings(&doc);
    let text = extract_text(html, max_chars);
    let len = text.as_ref().map(|t| t.len()).unwrap_or(0);

    Page {
        url: url.to_string(),
        ok: true,
        err: None,
        title,
        desc,
        text,
        h: headings,
        len,
    }
}

fn select_text(doc: &Html, sel: &str) -> Option<String> {
    let selector = Selector::parse(sel).ok()?;
    doc.select(&selector)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
}

fn select_attr(doc: &Html, sel: &str, attr: &str) -> Option<String> {
    let selector = Selector::parse(sel).ok()?;
    doc.select(&selector)
        .next()
        .and_then(|el| el.value().attr(attr))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_headings(doc: &Html) -> Vec<String> {
    let mut headings = Vec::new();
    for tag in ["h1", "h2", "h3"] {
        if let Ok(sel) = Selector::parse(tag) {
            for el in doc.select(&sel).take(10) {
                let text: String = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() && text.len() < 200 {
                    headings.push(text);
                }
            }
        }
    }
    headings.truncate(20);
    headings
}

fn extract_text(html: &str, max_chars: usize) -> Option<String> {
    let text = html2text::from_read(html.as_bytes(), 120).ok()?;

    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| l.len() > 2)
        .collect();

    let text = lines.join("\n");

    if text.is_empty() {
        return None;
    }

    let text = if max_chars > 0 && text.len() > max_chars {
        format!("{}...[truncated]", &text[..max_chars])
    } else {
        text
    };

    Some(text)
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
    fn test_parse_basic() {
        let html = r#"
            <html>
            <head><title>Test Page</title></head>
            <body>
                <h1>Main Title</h1>
                <p>Some content here.</p>
            </body>
            </html>
        "#;
        let page = parse(html, "https://test.com", 0);
        assert!(page.ok);
        assert_eq!(page.title, Some("Test Page".to_string()));
        assert!(page.text.unwrap().contains("content"));
    }

    #[test]
    fn test_extract_headings() {
        let html = "<h1>One</h1><h2>Two</h2><h3>Three</h3>";
        let doc = Html::parse_document(html);
        let h = extract_headings(&doc);
        assert_eq!(h.len(), 3);
        assert_eq!(h[0], "One");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is...");
    }
}
