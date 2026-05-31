#!/usr/bin/env node

/**
 * Elysium CLI alias (`ely`).
 *
 * Delegates to the same native binary as `elysium`.
 * This script is identical to elysium.js but registered under the `ely` bin name.
 *
 * Usage: ely <command> [options]
 *        ely run hello.ely
 *        ely check hello.ely
 */

const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');

const BIN_DIR = path.resolve(__dirname);
const BIN_NAME = process.platform === 'win32' ? 'elysium.exe' : 'elysium';
const BIN_PATH = path.join(BIN_DIR, BIN_NAME);

if (!fs.existsSync(BIN_PATH)) {
  console.error('Elysium binary not found. Try running `npm run postinstall` to rebuild.');
  console.error(`Expected at: ${BIN_PATH}`);
  process.exit(1);
}

const args = process.argv.slice(2);
const result = spawnSync(BIN_PATH, args, {
  stdio: 'inherit',
  env: process.env,
});

if (result.error) {
  console.error(`Failed to run Elysium: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 0);
