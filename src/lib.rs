//! ref-tools: Reference verification tools with headless Chrome
//!
//! Commands:
//! - check-links: Check URL health in markdown files
//! - refresh-data: Extract live data from URLs

pub mod browser;
pub mod check_links;
pub mod extract;
pub mod refresh_data;

pub use check_links::{check_links, CheckLinksConfig, LinkReport, LinkResult};
pub use refresh_data::{refresh_data, ExtractedData, RefreshConfig};
