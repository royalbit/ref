//! Headless Chrome browser management via chromiumoxide

use anyhow::{Context, Result};
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Browser pool configuration
pub struct BrowserPool {
    browser: Browser,
    semaphore: Arc<Semaphore>,
    user_agent: String,
}

impl BrowserPool {
    /// Create a new browser pool with concurrency limit
    pub async fn new(concurrency: usize) -> Result<Self> {
        let config = BrowserConfig::builder()
            .no_sandbox()
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-setuid-sandbox")
            .arg("--no-first-run")
            .arg("--headless=new")
            .build()
            .map_err(|e| anyhow::anyhow!("Browser config error: {}", e))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .context("Failed to launch Chrome. Is Chrome/Chromium installed?")?;

        // Spawn handler in background
        tokio::spawn(async move { while handler.next().await.is_some() {} });

        Ok(Self {
            browser,
            semaphore: Arc::new(Semaphore::new(concurrency)),
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
        })
    }

    /// Get a new page with resource blocking
    pub async fn new_page(&self) -> Result<BrowserPage> {
        let permit = self.semaphore.clone().acquire_owned().await?;
        let page = self.browser.new_page("about:blank").await?;

        // Set user agent
        page.execute(
            chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams::new(
                &self.user_agent,
            ),
        )
        .await?;

        Ok(BrowserPage {
            page,
            _permit: permit,
        })
    }

    /// Close the browser
    pub async fn close(mut self) -> Result<()> {
        self.browser.close().await?;
        Ok(())
    }
}

/// A browser page with automatic permit release
pub struct BrowserPage {
    page: Page,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl BrowserPage {
    /// Navigate to URL and wait for DOM content loaded
    pub async fn goto(&self, url: &str, timeout_ms: u64) -> Result<PageResult> {
        let nav_result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            self.page.goto(url),
        )
        .await;

        match nav_result {
            Ok(Ok(_)) => {
                let status = self.get_status().await;
                let title = self.page.get_title().await.ok().flatten();
                Ok(PageResult {
                    status,
                    title,
                    error: None,
                })
            }
            Ok(Err(e)) => {
                let (status, _) = parse_error(&e.to_string());
                Ok(PageResult {
                    status,
                    title: None,
                    error: Some(e.to_string()),
                })
            }
            Err(_) => Ok(PageResult {
                status: 0,
                title: None,
                error: Some("Navigation timeout".to_string()),
            }),
        }
    }

    /// Try to get HTTP status from the page (heuristic based on page content)
    async fn get_status(&self) -> u16 {
        // chromiumoxide doesn't expose HTTP status directly
        // We check if page loaded successfully by looking for error pages
        if let Ok(Some(t)) = self.page.get_title().await {
            let t_lower = t.to_lowercase();
            if t_lower.contains("404") || t_lower.contains("not found") {
                return 404;
            }
            if t_lower.contains("403")
                || t_lower.contains("forbidden")
                || t_lower.contains("access denied")
            {
                return 403;
            }
            if t_lower.contains("500") || t_lower.contains("internal server error") {
                return 500;
            }
        }
        // If we got here, assume success
        200
    }

    /// Get page content (for data extraction)
    pub async fn content(&self) -> Result<String> {
        self.page
            .content()
            .await
            .context("Failed to get page content")
    }

    /// Get current URL (after redirects)
    pub async fn current_url(&self) -> Option<String> {
        self.page.url().await.ok().flatten()
    }
}

/// Result of a page navigation
#[derive(Debug)]
pub struct PageResult {
    pub status: u16,
    pub title: Option<String>,
    pub error: Option<String>,
}

fn parse_error(error: &str) -> (u16, String) {
    if error.contains("ERR_NAME_NOT_RESOLVED") {
        (0, "DNS_FAILED".to_string())
    } else if error.contains("ERR_CONNECTION_REFUSED") {
        (0, "CONNECTION_REFUSED".to_string())
    } else if error.contains("ERR_CONNECTION_TIMED_OUT") {
        (0, "TIMEOUT".to_string())
    } else if error.contains("ERR_CERT") || error.contains("SSL") {
        (0, "SSL_ERROR".to_string())
    } else {
        (0, "NETWORK_ERROR".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        assert_eq!(parse_error("net::ERR_NAME_NOT_RESOLVED").1, "DNS_FAILED");
        assert_eq!(
            parse_error("ERR_CONNECTION_REFUSED").1,
            "CONNECTION_REFUSED"
        );
        assert_eq!(parse_error("random error").1, "NETWORK_ERROR");
    }
}
