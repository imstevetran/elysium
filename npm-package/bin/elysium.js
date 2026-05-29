#!/usr/bin/env node

/**
 * Elysium CLI launcher.
 *
 * Finds the native binary and spawns it with the given arguments.
 * The binary is downloaded during `npm install` or built from source.
 */

const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');

const BIN_DIR = path.resolve(__dirname);
const BIN_NAME = process.platform === 'win32' ? 'elysium.exe' : 'elysium';
const BIN_PATH = path.join(BIN_DIR, BIN_NAME);

// Check if the binary exists; if not, try building it
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
