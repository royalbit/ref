# @royalbit/ref-tools

Reference verification tools with headless Chrome.
Bypasses bot protection (403/999) that blocks curl/wget.

## Installation

```bash
# From source (recommended for development)
git clone crypto1.ca:pimp/tools ~/src/pimp/tools
cd ~/src/pimp/tools
npm install
npm link

# Verify installation
ref-tools --version
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

**Examples:**

```bash
# Check REFERENCES.md with default concurrency (5 tabs)
ref-tools check-links REFERENCES.md

# Fast mode - 15 parallel tabs
ref-tools check-links REFERENCES.md -c 15

# Check single URL
ref-tools check-links --url https://capterra.com/p/173654/GRIN/pricing/

# Pipe URLs from file
cat urls.txt | ref-tools check-links --stdin

# Save JSON report
ref-tools check-links REFERENCES.md > report.json
```

### refresh-data

Extract live data from URLs (market sizes, pricing, statistics).

```bash
ref-tools refresh-data --url <URL>        # Extract from single URL
ref-tools refresh-data                    # Process REFERENCES.md (TODO)
```

**Examples:**

```bash
# Extract market data from Statista
ref-tools refresh-data --url "https://www.statista.com/statistics/..."

# Extract pricing from Capterra
ref-tools refresh-data --url "https://www.capterra.com/p/173654/GRIN/pricing/"
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
  "url": "https://statista.com/...",
  "title": "Market Size Report",
  "extracted": {
    "amounts": ["$33 billion", "$48.2M"],
    "percentages": ["71%", "53%"]
  },
  "timestamp": "2025-12-07T..."
}
```

## Supported Extractions

| Source | Data Extracted |
|--------|----------------|
| **Statista** | Market size numbers ($XX billion) |
| **Capterra/G2** | Pricing info, reviews |
| **Generic** | Title, dollar amounts, percentages |

## Limitations

| Site | Issue | Workaround |
|------|-------|------------|
| Instagram | Requires login for follower counts | Manual verification |
| LinkedIn | Heavy bot protection | Manual verification |
| G2.com | Cloudflare challenge | Usually passes, retry if blocked |
| Paywalled | Only free content accessible | N/A |

## Why Headless Chrome?

Many sites block simple HTTP requests:

| Method | Capterra | G2 | PitchBook |
|--------|----------|-----|-----------|
| curl/wget | 403 | 403 | 403 |
| Headless Chrome | 200 | 200* | 200 |

*G2 occasionally triggers Cloudflare challenges.

Puppeteer with a real Chrome browser and proper user-agent bypasses most bot protection.

## Development

```bash
# Run locally without npm link
node bin/cli.js check-links ./test.md

# Run specific command directly
node bin/check-links.js ./test.md -c 10

# Debug mode (shows browser)
# Edit bin/check-links.js: CONFIG.headless = false
```

## Project Structure

```
pimp/tools/
├── bin/
│   ├── cli.js           # Main CLI entry point
│   ├── check-links.js   # Link health checker
│   └── refresh-data.js  # Data extractor
├── package.json
└── README.md
```

## Related

- `pimp/business` - Business docs using these tools
- `pimp/arch` - Technical architecture

## License

MIT - RoyalBit
