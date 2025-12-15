//! ref-tools: LLM-optimized web fetching
//!
//! JSON output for agents, not humans.
//! Bypasses bot protection via headless Chrome.

pub mod browser;
pub mod check_links;
pub mod extract;
pub mod refresh_data;
pub mod schema;
pub mod verify_refs;

pub use check_links::{check_links, CheckLinksConfig, LinkReport, LinkResult};
pub use refresh_data::{refresh_data, ExtractedData, RefreshConfig};
pub use schema::{Reference, ReferencesFile, Status};
