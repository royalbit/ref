#!/usr/bin/env node
/**
 * Headless Chrome Link Checker
 *
 * Uses Puppeteer to check URLs with a real browser, bypassing bot protection.
 *
 * Usage:
 *   check-links <file.md>              Check URLs in a markdown file
 *   check-links --url <URL>            Check a single URL
 *   check-links --stdin                Read URLs from stdin (one per line)
 *   check-links <file.md> -c 10        Run with 10 parallel browser tabs
 *
 * Options:
 *   -c, --concurrency <N>   Number of parallel browser tabs (default: 5)
 *
 * Output:
 *   JSON report to stdout with status for each URL
 */

import puppeteer from 'puppeteer';
import fs from 'fs';
import path from 'path';

// Configuration
const CONFIG = {
  concurrency: 5,           // Parallel browser pages
  timeout: 15000,           // Page load timeout (ms)
  userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
  headless: true,
  retries: 1,
};

// Extract URLs from markdown content
function extractUrls(content) {
  const urlPattern = /https?:\/\/[^\s\)>\]"']+/g;
  const matches = content.match(urlPattern) || [];
  // Deduplicate and clean
  return [...new Set(matches.map(url => url.replace(/[,.\)]+$/, '')))];
}

// Check a single URL with Puppeteer
async function checkUrl(page, url) {
  const result = {
    url,
    status: null,
    statusText: null,
    title: null,
    error: null,
    time: null,
  };

  const start = Date.now();

  try {
    const response = await page.goto(url, {
      waitUntil: 'domcontentloaded',
      timeout: CONFIG.timeout,
    });

    result.status = response?.status() || 0;
    result.statusText = response?.statusText() || 'Unknown';
    result.title = await page.title().catch(() => null);
    result.time = Date.now() - start;

  } catch (err) {
    result.error = err.message;
    result.time = Date.now() - start;

    // Try to extract status from error message
    if (err.message.includes('net::ERR_NAME_NOT_RESOLVED')) {
      result.status = 0;
      result.statusText = 'DNS_FAILED';
    } else if (err.message.includes('net::ERR_CONNECTION_REFUSED')) {
      result.status = 0;
      result.statusText = 'CONNECTION_REFUSED';
    } else if (err.message.includes('net::ERR_CONNECTION_TIMED_OUT')) {
      result.status = 0;
      result.statusText = 'TIMEOUT';
    } else if (err.message.includes('Navigation timeout')) {
      result.status = 0;
      result.statusText = 'NAV_TIMEOUT';
    }
  }

  return result;
}

// Process URLs with concurrency limit
async function processUrls(urls, browser) {
  const results = [];
  const queue = [...urls];
  const workers = [];

  for (let i = 0; i < CONFIG.concurrency; i++) {
    workers.push((async () => {
      const page = await browser.newPage();
      await page.setUserAgent(CONFIG.userAgent);
      await page.setViewport({ width: 1280, height: 720 });

      // Block unnecessary resources for speed
      await page.setRequestInterception(true);
      page.on('request', (req) => {
        const type = req.resourceType();
        if (['image', 'stylesheet', 'font', 'media'].includes(type)) {
          req.abort();
        } else {
          req.continue();
        }
      });

      while (queue.length > 0) {
        const url = queue.shift();
        if (!url) break;

        process.stderr.write(`Checking: ${url.substring(0, 60)}...\n`);

        let result = await checkUrl(page, url);

        // Retry once if failed
        if (result.status === 0 && CONFIG.retries > 0) {
          await new Promise(r => setTimeout(r, 1000));
          result = await checkUrl(page, url);
        }

        results.push(result);
      }

      await page.close();
    })());
  }

  await Promise.all(workers);
  return results;
}

// Generate summary report
function generateReport(results) {
  const summary = {
    total: results.length,
    ok: 0,
    redirects: 0,
    clientErrors: 0,
    serverErrors: 0,
    blocked: 0,
    failed: 0,
  };

  const byStatus = {};

  for (const r of results) {
    const status = r.status || 0;
    byStatus[status] = (byStatus[status] || 0) + 1;

    if (status >= 200 && status < 300) summary.ok++;
    else if (status >= 300 && status < 400) summary.redirects++;
    else if (status === 403) summary.blocked++;
    else if (status >= 400 && status < 500) summary.clientErrors++;
    else if (status >= 500) summary.serverErrors++;
    else summary.failed++;
  }

  return {
    summary,
    byStatus,
    results: results.sort((a, b) => (a.status || 999) - (b.status || 999)),
    timestamp: new Date().toISOString(),
  };
}

// Parse CLI argument
function getArg(args, ...flags) {
  for (const flag of flags) {
    const idx = args.indexOf(flag);
    if (idx !== -1 && args[idx + 1]) {
      return args[idx + 1];
    }
  }
  return null;
}

// Main
async function main() {
  const args = process.argv.slice(2);
  let urls = [];

  // Parse concurrency option
  const concurrencyArg = getArg(args, '-c', '--concurrency');
  if (concurrencyArg) {
    const n = parseInt(concurrencyArg, 10);
    if (n > 0 && n <= 20) {
      CONFIG.concurrency = n;
    } else {
      console.error('Concurrency must be between 1 and 20');
      process.exit(1);
    }
  }

  // Parse input source
  if (args.includes('--url')) {
    const idx = args.indexOf('--url');
    urls = [args[idx + 1]];
  } else if (args.includes('--stdin')) {
    const input = fs.readFileSync(0, 'utf-8');
    urls = input.split('\n').filter(u => u.trim().startsWith('http'));
  } else {
    // Find first non-flag argument as file path
    const file = args.find(a => !a.startsWith('-') && a !== concurrencyArg);
    if (file && fs.existsSync(file)) {
      const content = fs.readFileSync(file, 'utf-8');
      urls = extractUrls(content);
    } else {
      console.error('Usage:');
      console.error('  check-links <file.md>              Check URLs in markdown file');
      console.error('  check-links --url <URL>            Check single URL');
      console.error('  check-links --stdin                Read URLs from stdin');
      console.error('');
      console.error('Options:');
      console.error('  -c, --concurrency <N>   Parallel browser tabs (default: 5, max: 20)');
      process.exit(1);
    }
  }

  if (urls.length === 0) {
    console.error('No URLs found.');
    process.exit(1);
  }

  process.stderr.write(`Found ${urls.length} URLs to check (concurrency: ${CONFIG.concurrency})\n\n`);

  // Launch browser
  const browser = await puppeteer.launch({
    headless: CONFIG.headless,
    args: [
      '--no-sandbox',
      '--disable-setuid-sandbox',
      '--disable-dev-shm-usage',
      '--disable-accelerated-2d-canvas',
      '--disable-gpu',
    ],
  });

  try {
    const results = await processUrls(urls, browser);
    const report = generateReport(results);

    // Output JSON report
    console.log(JSON.stringify(report, null, 2));

    // Print summary to stderr
    process.stderr.write('\n--- SUMMARY ---\n');
    process.stderr.write(`Total:    ${report.summary.total}\n`);
    process.stderr.write(`OK (2xx): ${report.summary.ok}\n`);
    process.stderr.write(`Blocked:  ${report.summary.blocked}\n`);
    process.stderr.write(`Errors:   ${report.summary.clientErrors + report.summary.serverErrors}\n`);
    process.stderr.write(`Failed:   ${report.summary.failed}\n`);

  } finally {
    await browser.close();
  }
}

main().catch(err => {
  console.error('Fatal error:', err.message);
  process.exit(1);
});
