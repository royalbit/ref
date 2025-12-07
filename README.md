# @royalbit/ref-tools

Reference verification tools with headless Chrome - bypasses bot protection (403/999).

## Installation

```bash
# Global install
npm install -g @royalbit/ref-tools

# Or use npx (no install needed)
npx @royalbit/ref-tools check-links ./REFERENCES.md
```

## Usage

### Check Links

Check URL health in a markdown file:

```bash
ref-tools check-links ./REFERENCES.md
ref-tools check-links --url https://capterra.com/p/173654/GRIN/pricing/
```

### Refresh Data

Extract live data from URLs (market sizes, pricing, etc.):

```bash
ref-tools refresh-data --url "https://www.statista.com/..."
ref-tools refresh-data              # Process all extractable URLs in REFERENCES.md
```

## Output

- **JSON report** to stdout (pipe to file)
- **Progress and summary** to stderr

```bash
ref-tools check-links ./REFERENCES.md > report.json
```

## Supported Extractions

| Source | Data Extracted |
|--------|----------------|
| **Statista** | Market size numbers ($XX billion) |
| **Capterra/G2** | Pricing info, reviews |
| **Generic** | Title, dollar amounts, percentages |

## Limitations

- **Instagram:** Requires login to see follower counts
- **LinkedIn:** Heavy bot protection
- **Paywalled sites:** Only free content accessible

## Why Headless Chrome?

Many sites (Capterra, G2, PitchBook) block curl/wget with 403.
A real browser with proper user-agent bypasses most protections.

## License

MIT - RoyalBit
