# ref-tools

Reference verification tools with headless Chrome.
Bypasses bot protection (403/999) that blocks curl/wget.

## Installation

```bash
# Build from source
git clone <repo> && cd tools
make build

# Install to ~/.local/bin
make install
```

## Commands

### check-links

Check URL health in markdown files or single URLs.

```bash
ref-tools check-links <file.md>           # Check all URLs in file
ref-tools check-links --url <URL>         # Check single URL
ref-tools check-links --stdin             # Read URLs from stdin
```

**Options:**

| Flag | Description |
|------|-------------|
| `-c, --concurrency <N>` | Parallel browser tabs (1-20, default: 5) |
| `--timeout <MS>` | Timeout per URL in milliseconds (default: 15000) |
| `--retries <N>` | Number of retries on failure (default: 1) |

**Examples:**

```bash
# Check REFERENCES.md with default concurrency (5 tabs)
ref-tools check-links REFERENCES.md

# Fast mode - 15 parallel tabs
ref-tools check-links REFERENCES.md -c 15

# Check single URL
ref-tools check-links --url https://example.com

# Pipe URLs from file
cat urls.txt | ref-tools check-links --stdin
```

### refresh-data

Extract live data from URLs (market sizes, pricing, statistics).

```bash
ref-tools refresh-data --url <URL>        # Extract from single URL
ref-tools refresh-data <file.md>          # Process URLs in file
ref-tools refresh-data <file.md> --filter instagram
```

**Examples:**

```bash
# Extract market data from Statista
ref-tools refresh-data --url "https://www.statista.com/statistics/..."

# Extract only Instagram data from references
ref-tools refresh-data REFERENCES.md --filter instagram
```

## Output Format

Both commands output JSON to stdout, progress to stderr.

### check-links output

```json
{
  "summary": {
    "total": 126,
    "ok": 121,
    "redirects": 0,
    "clientErrors": 1,
    "serverErrors": 0,
    "blocked": 4,
    "failed": 0
  },
  "byStatus": {
    "200": 121,
    "403": 4,
    "404": 1
  },
  "results": [
    {
      "url": "https://example.com",
      "status": 200,
      "statusText": "OK",
      "title": "Page Title",
      "time": 1234
    }
  ],
  "timestamp": "2025-12-07T..."
}
```

### refresh-data output

```json
{
  "results": [
    {
      "url": "https://statista.com/...",
      "type": "statista",
      "success": true,
      "title": "Market Size Report",
      "amounts": [{"value": "33", "unit": "billion", "raw": "$33 billion"}],
      "percentages": ["71%", "53%"]
    }
  ],
  "timestamp": "2025-12-07T..."
}
```

## Supported Extractors

| Source | Data Extracted |
|--------|----------------|
| **Statista** | Market size numbers, percentages |
| **Instagram** | Follower counts |
| **Generic** | Title, description, dollar amounts, percentages |

## Requirements

- Chrome or Chromium installed (for headless browsing)
- Rust toolchain (for building from source)

## Development

```bash
# Build
make build

# Run tests
make test

# Run with pedantic lints
make lint

# Format code
make format
```

## Project Structure

```
ref-tools/
├── src/
│   ├── main.rs          # CLI entry point
│   ├── lib.rs           # Library root
│   ├── browser.rs       # Headless Chrome via chromiumoxide
│   ├── check_links.rs   # check-links command
│   ├── extract.rs       # URL/data extraction utilities
│   └── refresh_data.rs  # refresh-data command
├── tests/
│   └── cli_tests.rs     # E2E tests
├── test-data/
│   └── sample.md        # Test fixtures
├── Cargo.toml
├── Makefile
└── README.md
```

## License

MIT - RoyalBit
