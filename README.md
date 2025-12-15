# ref-tools

LLM-optimized reference tools. JSON output for AI agents, not humans.

Bypasses bot protection (403/999) via headless Chrome.

## Why

```bash
# curl gets blocked
curl https://linkedin.com/...  # 999 Request Denied

# ref-tools gets through
ref-tools fetch https://linkedin.com/...  # JSON with content
```

## Install

```bash
make build && make install  # → ~/.local/bin/ref-tools
```

## Commands

### fetch

Fetch URL content as structured JSON.

```bash
ref-tools fetch <url>
```

### pdf

Extract text from PDF files to structured JSON.

```bash
ref-tools pdf document.pdf
ref-tools pdf *.pdf  # Multiple files
```

### init

Create a new references.yaml template.

```bash
ref-tools init                    # Creates references.yaml
ref-tools init -o refs.yaml       # Custom filename
```

### scan

Scan markdown files for URLs, build/update references.yaml.

```bash
ref-tools scan README.md docs/*.md
ref-tools scan . --output refs.yaml
```

### verify-refs

Verify references.yaml entries, update status.

```bash
ref-tools verify-refs references.yaml
ref-tools verify-refs references.yaml --category research
ref-tools verify-refs references.yaml --parallel 10
```

### check-links

Check URL health. Returns status codes.

```bash
ref-tools check-links <file.md>       # All URLs in file
ref-tools check-links --url <URL>     # Single URL
ref-tools check-links --stdin         # From stdin
```

### refresh-data

Extract structured data (market sizes, stats, follower counts).

```bash
ref-tools refresh-data --url <URL>
```

## Output

All commands: JSON to stdout, logs to stderr.

```bash
ref-tools fetch https://example.com 2>/dev/null | jq .
```

## Requirements

- Chrome/Chromium (headless)
- Rust toolchain (build only)

## License

MIT - RoyalBit
