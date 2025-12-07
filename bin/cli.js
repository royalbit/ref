#!/usr/bin/env node
/**
 * @royalbit/ref-tools CLI
 *
 * Usage:
 *   ref-tools check-links <file.md>     Check URL health in markdown
 *   ref-tools refresh-data [options]    Extract live data from URLs
 *   ref-tools --help                    Show help
 */

import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));

const commands = {
  'check-links': join(__dirname, 'check-links.js'),
  'refresh-data': join(__dirname, 'refresh-data.js'),
};

const args = process.argv.slice(2);
const command = args[0];

if (!command || command === '--help' || command === '-h') {
  console.log(`
@royalbit/ref-tools - Reference verification tools

Usage:
  ref-tools check-links <file.md>           Check URL health in markdown
  ref-tools check-links --url <URL>         Check single URL
  ref-tools check-links <file.md> -c 10     Run with 10 parallel tabs
  ref-tools refresh-data                    Extract data from URLs
  ref-tools refresh-data --url <URL>        Extract from single URL

Examples:
  ref-tools check-links ./REFERENCES.md
  ref-tools check-links ./REFERENCES.md --concurrency 15
  ref-tools check-links --url https://capterra.com/...
  ref-tools refresh-data --url https://statista.com/...

Options:
  -c, --concurrency <N>   Parallel browser tabs (default: 5, max: 20)
  --help, -h              Show this help message
  --version               Show version
`);
  process.exit(0);
}

if (command === '--version') {
  const pkg = await import('../package.json', { assert: { type: 'json' } });
  console.log(pkg.default.version);
  process.exit(0);
}

const script = commands[command];
if (!script) {
  console.error(`Unknown command: ${command}`);
  console.error('Run "ref-tools --help" for usage');
  process.exit(1);
}

// Run the command with remaining args
const child = spawn('node', [script, ...args.slice(1)], {
  stdio: 'inherit',
  cwd: process.cwd(),
});

child.on('exit', (code) => {
  process.exit(code || 0);
});
