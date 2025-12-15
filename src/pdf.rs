//! pdf command: Extract text from PDF files
//!
//! Local extraction, no external APIs.
//! Output matches fetch command structure for consistency.

use crate::fetch::{CodeBlock, Link, Page, PageStatus, Section};
use anyhow::Result;
use clap::Args;
use regex::Regex;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct PdfArgs {
    /// PDF files to extract
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

pub async fn run_pdf(args: PdfArgs) -> Result<()> {
    let file_count = args.files.len();
    eprintln!(
        "Extracting {} PDF{}...",
        file_count,
        if file_count == 1 { "" } else { "s" }
    );

    let mut results: Vec<Page> = Vec::new();

    for file in &args.files {
        eprintln!("  -> {}", file.display());
        let page = extract_pdf(file).await;
        results.push(page);
    }

    let ok_count = results
        .iter()
        .filter(|p| p.status == PageStatus::Ok)
        .count();

    // Output compact JSON (one line per page for multiple, or single object)
    if results.len() == 1 {
        println!("{}", serde_json::to_string(&results[0])?);
    } else {
        for page in &results {
            println!("{}", serde_json::to_string(page)?);
        }
    }

    eprintln!("Done: {}/{} OK", ok_count, file_count);
    Ok(())
}

async fn extract_pdf(path: &PathBuf) -> Page {
    let file_url = format!("file://{}", path.display());

    // Check file exists
    if !path.exists() {
        return error_page(&file_url, "File not found");
    }

    // Extract text using pdf-extract
    let text = match pdf_extract::extract_text(path) {
        Ok(t) => t,
        Err(e) => {
            return error_page(&file_url, &format!("PDF extraction failed: {}", e));
        }
    };

    if text.is_empty() {
        return error_page(&file_url, "PDF contains no extractable text");
    }

    // Parse the extracted text into sections
    let sections = parse_sections(&text);

    // Extract any URLs from the text
    let links = extract_links(&text);

    // Try to extract title from first line or filename
    let title = extract_title(&text, path);

    // Calculate total chars
    let chars: usize = sections
        .iter()
        .map(|s| s.content.len() + s.heading.len())
        .sum();

    Page {
        url: file_url,
        status: PageStatus::Ok,
        title,
        site: None,
        author: extract_author(&text),
        date: extract_date(&text),
        doi: extract_doi(&text),
        sections,
        links,
        code: extract_code(&text),
        alerts: vec![],
        chars,
    }
}

fn error_page(url: &str, error: &str) -> Page {
    Page {
        url: url.to_string(),
        status: PageStatus::Dead,
        title: None,
        site: None,
        author: None,
        date: None,
        doi: None,
        sections: vec![],
        links: vec![],
        code: vec![],
        alerts: vec![error.to_string()],
        chars: 0,
    }
}

fn parse_sections(text: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    // Heuristic: uppercase lines or lines ending with numbers might be headings
    let heading_re =
        Regex::new(r"^(?:(?:\d+\.?\s+)?[A-Z][A-Z\s]+$|(?:Chapter|Section|Part)\s+\d+)").unwrap();

    let mut current_heading = "Content".to_string();
    let mut current_content = String::new();
    let mut current_level: u8 = 1;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !current_content.is_empty() {
                current_content.push_str("\n\n");
            }
            continue;
        }

        // Check if this looks like a heading
        let is_heading = heading_re.is_match(trimmed)
            || (trimmed.len() < 100
                && trimmed.chars().filter(|c| c.is_uppercase()).count() > trimmed.len() / 2
                && trimmed.len() > 3);

        if is_heading && current_content.len() > 50 {
            // Save current section
            sections.push(Section {
                level: current_level,
                heading: truncate(&current_heading, 200),
                content: truncate(current_content.trim(), 10000),
            });

            // Start new section
            current_heading = trimmed.to_string();
            current_content = String::new();

            // Guess level from numbering
            current_level = if trimmed.starts_with(char::is_numeric) {
                let dots = trimmed
                    .chars()
                    .take_while(|c| c.is_numeric() || *c == '.')
                    .filter(|c| *c == '.')
                    .count();
                (dots + 1).min(6) as u8
            } else {
                1
            };
        } else {
            if !current_content.is_empty() && !current_content.ends_with('\n') {
                current_content.push(' ');
            }
            current_content.push_str(trimmed);
        }
    }

    // Add final section
    if !current_content.is_empty() {
        sections.push(Section {
            level: current_level,
            heading: truncate(&current_heading, 200),
            content: truncate(current_content.trim(), 10000),
        });
    }

    // Limit sections
    sections.truncate(100);
    sections
}

fn extract_title(text: &str, path: &Path) -> Option<String> {
    // Try first non-empty line
    let first_line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string());

    // If first line is reasonable length, use it
    if let Some(ref line) = first_line {
        if line.len() < 200 && line.len() > 3 {
            return Some(truncate(line, 200));
        }
    }

    // Fallback to filename
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

fn extract_author(text: &str) -> Option<String> {
    let author_re = Regex::new(r"(?i)(?:author|by|written by)[:\s]+([^\n]+)").ok()?;
    author_re
        .captures(text)
        .map(|c| c[1].trim().to_string())
        .filter(|s| s.len() < 200)
}

fn extract_date(text: &str) -> Option<String> {
    // Common date patterns
    let date_re = Regex::new(
        r"(?i)(?:date|published|updated)[:\s]+(\d{4}[-/]\d{2}[-/]\d{2}|\w+\s+\d{1,2},?\s+\d{4})",
    )
    .ok()?;
    date_re.captures(text).map(|c| c[1].trim().to_string())
}

fn extract_doi(text: &str) -> Option<String> {
    let doi_re = Regex::new(r"(?i)(?:doi[:\s]+|https?://doi\.org/)(10\.\d{4,}/[^\s\)]+)").ok()?;
    doi_re.captures(text).map(|c| c[1].trim().to_string())
}

fn extract_links(text: &str) -> Vec<Link> {
    let url_re = Regex::new(r#"https?://[^\s\)>\]"']+"#).unwrap();
    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for mat in url_re.find_iter(text) {
        let url = mat
            .as_str()
            .trim_end_matches([',', '.', ')', ']', ';', ':']);
        if !seen.contains(url) {
            seen.insert(url.to_string());
            links.push(Link {
                text: truncate(url, 100),
                url: url.to_string(),
            });
        }
        if links.len() >= 50 {
            break;
        }
    }

    links
}

fn extract_code(text: &str) -> Vec<CodeBlock> {
    // Look for code-like patterns (indented blocks, common code patterns)
    let mut blocks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();

    let mut in_code_block = false;
    let mut code_lines = Vec::new();

    for line in lines {
        // Heuristic: 4+ space indent or tab, or contains code-like patterns
        let is_code_line = line.starts_with("    ")
            || line.starts_with('\t')
            || (line.contains("def ")
                || line.contains("fn ")
                || line.contains("function ")
                || line.contains("class ")
                || line.contains("import ")
                || line.contains("package ")
                || line.contains("//")
                || line.contains("/*")
                || line.contains("#include"));

        if is_code_line {
            in_code_block = true;
            code_lines.push(line.to_string());
        } else if in_code_block {
            if code_lines.len() >= 3 {
                let source = code_lines.join("\n");
                if source.len() >= 20 {
                    blocks.push(CodeBlock {
                        lang: detect_language(&source),
                        source: truncate(&source, 5000),
                    });
                }
            }
            code_lines.clear();
            in_code_block = false;
        }

        if blocks.len() >= 10 {
            break;
        }
    }

    // Don't forget trailing code block
    if code_lines.len() >= 3 {
        let source = code_lines.join("\n");
        if source.len() >= 20 {
            blocks.push(CodeBlock {
                lang: detect_language(&source),
                source: truncate(&source, 5000),
            });
        }
    }

    blocks
}

fn detect_language(code: &str) -> Option<String> {
    if code.contains("fn ") && code.contains("->") {
        Some("rust".to_string())
    } else if code.contains("def ") && code.contains(":") {
        Some("python".to_string())
    } else if code.contains("function ") || code.contains("const ") || code.contains("let ") {
        Some("javascript".to_string())
    } else if code.contains("public class") || code.contains("private ") {
        Some("java".to_string())
    } else if code.contains("#include") {
        Some("c".to_string())
    } else {
        None
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
    fn test_extract_doi() {
        let text = "This paper (DOI: 10.1234/abc.123) presents...";
        assert_eq!(extract_doi(text), Some("10.1234/abc.123".to_string()));
    }

    #[test]
    fn test_extract_links() {
        let text = "See https://example.com and https://foo.bar/path for details.";
        let links = extract_links(text);
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(
            detect_language("fn main() -> i32 { 42 }"),
            Some("rust".to_string())
        );
        assert_eq!(
            detect_language("def foo(): pass"),
            Some("python".to_string())
        );
    }
}
