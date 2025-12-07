#!/usr/bin/env node
/**
 * Reference Data Refresher
 *
 * Uses headless Chrome to extract live data from URLs and compare
 * against claims in REFERENCES.md.
 *
 * Usage:
 *   node refresh-data.js                    # Check all extractable URLs
 *   node refresh-data.js --instagram        # Check Instagram profiles
 *   node refresh-data.js --url <URL>        # Extract data from single URL
 *
 * Supported extractions:
 *   - Instagram: follower count
 *   - Statista: market size numbers
 *   - LinkedIn: company info
 *   - Generic: page title, meta description
 */

import puppeteer from 'puppeteer';
import fs from 'fs';

const CONFIG = {
  timeout: 20000,
  userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
};

// Extractors for different site types
const EXTRACTORS = {
  // Instagram profile - get follower count
  instagram: async (page, url) => {
    await page.goto(url, { waitUntil: 'networkidle2', timeout: CONFIG.timeout });

    // Wait for content to load
    await page.waitForSelector('header', { timeout: 5000 }).catch(() => {});

    const data = await page.evaluate(() => {
      // Try multiple selectors for follower count
      const text = document.body.innerText;

      // Pattern: "577K followers" or "1.2M followers"
      const followerMatch = text.match(/([0-9,.]+[KMB]?)\s*followers/i);

      // Get username from URL or meta
      const username = document.querySelector('meta[property="og:title"]')?.content?.split(' ')[0] ||
                       window.location.pathname.replace(/\//g, '');

      return {
        username,
        followers: followerMatch ? followerMatch[1] : null,
        fullText: followerMatch ? followerMatch[0] : null,
      };
    });

    return {
      type: 'instagram',
      ...data,
      url,
    };
  },

  // Statista - extract market size numbers
  statista: async (page, url) => {
    await page.goto(url, { waitUntil: 'domcontentloaded', timeout: CONFIG.timeout });

    const data = await page.evaluate(() => {
      const text = document.body.innerText;

      // Look for billion/million dollar amounts
      const amounts = [];
      const patterns = [
        /\$([0-9,.]+)\s*(billion|million|B|M)/gi,
        /([0-9,.]+)\s*(billion|million)\s*U\.?S\.?\s*dollars/gi,
      ];

      for (const pattern of patterns) {
        let match;
        while ((match = pattern.exec(text)) !== null) {
          amounts.push({
            value: match[1],
            unit: match[2],
            context: text.substring(Math.max(0, match.index - 50), match.index + match[0].length + 50),
          });
        }
      }

      const title = document.querySelector('h1')?.innerText || document.title;

      return { title, amounts: amounts.slice(0, 5) }; // Top 5 matches
    });

    return {
      type: 'statista',
      ...data,
      url,
    };
  },

  // Generic page - title, meta, key numbers
  generic: async (page, url) => {
    await page.goto(url, { waitUntil: 'domcontentloaded', timeout: CONFIG.timeout });

    const data = await page.evaluate(() => {
      const title = document.querySelector('h1')?.innerText || document.title;
      const description = document.querySelector('meta[name="description"]')?.content || '';
      const text = document.body.innerText;

      // Extract dollar amounts
      const dollarPattern = /\$([0-9,.]+)\s*(billion|million|B|M|K)?/gi;
      const amounts = [];
      let match;
      while ((match = dollarPattern.exec(text)) !== null && amounts.length < 5) {
        amounts.push({
          value: match[1],
          unit: match[2] || '',
          raw: match[0],
        });
      }

      // Extract percentages
      const percentPattern = /([0-9,.]+)\s*%/g;
      const percentages = [];
      while ((match = percentPattern.exec(text)) !== null && percentages.length < 5) {
        percentages.push(match[0]);
      }

      return { title, description, amounts, percentages };
    });

    return {
      type: 'generic',
      ...data,
      url,
    };
  },
};

// Determine which extractor to use based on URL
function getExtractor(url) {
  if (url.includes('instagram.com')) return 'instagram';
  if (url.includes('statista.com')) return 'statista';
  return 'generic';
}

// Extract data from a single URL
async function extractData(browser, url) {
  const page = await browser.newPage();
  await page.setUserAgent(CONFIG.userAgent);
  await page.setViewport({ width: 1280, height: 720 });

  // Block images/media for speed
  await page.setRequestInterception(true);
  page.on('request', (req) => {
    const type = req.resourceType();
    if (['image', 'media', 'font'].includes(type)) {
      req.abort();
    } else {
      req.continue();
    }
  });

  const extractorName = getExtractor(url);
  const extractor = EXTRACTORS[extractorName];

  try {
    process.stderr.write(`Extracting [${extractorName}]: ${url.substring(0, 60)}...\n`);
    const result = await extractor(page, url);
    result.success = true;
    result.timestamp = new Date().toISOString();
    return result;
  } catch (err) {
    return {
      url,
      type: extractorName,
      success: false,
      error: err.message,
      timestamp: new Date().toISOString(),
    };
  } finally {
    await page.close();
  }
}

// Extract URLs from REFERENCES.md that we can process
function getExtractableUrls(content) {
  const urls = [];
  const patterns = [
    // Instagram profiles
    { pattern: /https?:\/\/(?:www\.)?instagram\.com\/[^\s\)\]]+/g, type: 'instagram' },
    // Statista
    { pattern: /https?:\/\/(?:www\.)?statista\.com\/[^\s\)\]]+/g, type: 'statista' },
    // Key competitor/market pages
    { pattern: /https?:\/\/(?:www\.)?(influencermarketinghub|emarketer|techcrunch)\.com\/[^\s\)\]]+/g, type: 'market' },
  ];

  for (const { pattern, type } of patterns) {
    const matches = content.match(pattern) || [];
    for (const url of matches) {
      urls.push({ url: url.replace(/[,.\)]+$/, ''), type });
    }
  }

  return [...new Map(urls.map(u => [u.url, u])).values()]; // Dedupe
}

// Main
async function main() {
  const args = process.argv.slice(2);
  let urls = [];

  if (args.includes('--url')) {
    const idx = args.indexOf('--url');
    urls = [{ url: args[idx + 1], type: getExtractor(args[idx + 1]) }];
  } else if (args.includes('--instagram')) {
    // Extract only Instagram URLs from REFERENCES.md
    const content = fs.readFileSync('../REFERENCES.md', 'utf-8');
    urls = getExtractableUrls(content).filter(u => u.type === 'instagram');
  } else {
    // Extract all processable URLs from REFERENCES.md
    const content = fs.readFileSync('../REFERENCES.md', 'utf-8');
    urls = getExtractableUrls(content);
  }

  if (urls.length === 0) {
    console.error('No extractable URLs found.');
    process.exit(1);
  }

  process.stderr.write(`Found ${urls.length} URLs to extract data from\n\n`);

  const browser = await puppeteer.launch({
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox'],
  });

  const results = [];

  try {
    for (const { url } of urls) {
      const result = await extractData(browser, url);
      results.push(result);

      // Rate limit
      await new Promise(r => setTimeout(r, 1000));
    }

    console.log(JSON.stringify({ results, timestamp: new Date().toISOString() }, null, 2));

    // Summary to stderr
    process.stderr.write('\n--- EXTRACTION SUMMARY ---\n');
    const success = results.filter(r => r.success).length;
    process.stderr.write(`Success: ${success}/${results.length}\n`);

    // Show Instagram results
    const instagram = results.filter(r => r.type === 'instagram' && r.success);
    if (instagram.length > 0) {
      process.stderr.write('\nInstagram Follower Counts:\n');
      for (const r of instagram) {
        process.stderr.write(`  @${r.username}: ${r.followers || 'N/A'}\n`);
      }
    }

  } finally {
    await browser.close();
  }
}

main().catch(err => {
  console.error('Fatal error:', err.message);
  process.exit(1);
});
