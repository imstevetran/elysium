#!/usr/bin/env node

/**
 * Build Elysium binaries for npm release.
 *
 * Cross-compiles the elysium binary for multiple targets and gzips
 * them individually for GitHub Releases.
 *
 * Usage:
 *   node scripts/build-binaries.js             # build for current platform
 *   node scripts/build-binaries.js --all       # build for all platforms
 *
 * Requires Rust cross-compilation targets:
 *   rustup target add x86_64-apple-darwin aarch64-apple-darwin \
 *                      x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu \
 *                      x86_64-pc-windows-msvc
 *
 * Zero external dependencies — only Node.js built-ins.
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const PKG_VERSION = require('../package.json').version;
const ROOT_DIR = path.resolve(__dirname, '..', '..');
const OUT_DIR = path.resolve(__dirname, '..', 'dist');

const TARGETS = [
  'x86_64-apple-darwin',
  'aarch64-apple-darwin',
  'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu',
  'x86_64-pc-windows-msvc',
];

const TARGET_SHORT = {
  'x86_64-apple-darwin': 'x86_64-apple-darwin',
  'aarch64-apple-darwin': 'aarch64-apple-darwin',
  'x86_64-unknown-linux-gnu': 'x86_64-unknown-linux-gnu',
  'aarch64-unknown-linux-gnu': 'aarch64-unknown-linux-gnu',
  'x86_64-pc-windows-msvc': 'x86_64-pc-windows-msvc',
};

function getHostTarget() {
  const os = process.platform;
  const arch = process.arch;
  if (os === 'darwin' && arch === 'x64') return 'x86_64-apple-darwin';
  if (os === 'darwin' && arch === 'arm64') return 'aarch64-apple-darwin';
  if (os === 'linux' && arch === 'x64') return 'x86_64-unknown-linux-gnu';
  if (os === 'linux' && arch === 'arm64') return 'aarch64-unknown-linux-gnu';
  if (os === 'win32' && arch === 'x64') return 'x86_64-pc-windows-msvc';
  throw new Error(`Unsupported host platform: ${os} ${arch}`);
}

function cargoBuild(target) {
  console.log(`Building for ${target}...`);
  execSync(`cargo build --release --target ${target} --bin elysium`, {
    cwd: ROOT_DIR,
    stdio: 'inherit',
  });
}

function gzipBinary(target, binName) {
  const targetDir = path.join(ROOT_DIR, 'target', target, 'release');
  const binPath = path.join(targetDir, binName);

  if (!fs.existsSync(binPath)) {
    throw new Error(`Binary not found: ${binPath}`);
  }

  const archiveName = `elysium-${PKG_VERSION}-${TARGET_SHORT[target]}.gz`;
  const archivePath = path.join(OUT_DIR, archiveName);

  fs.mkdirSync(OUT_DIR, { recursive: true });

  const binData = fs.readFileSync(binPath);
  const gzipped = zlib.gzipSync(binData);
  fs.writeFileSync(archivePath, gzipped);

  console.log(`  -> ${archivePath} (${(gzipped.length / 1024 / 1024).toFixed(1)} MB)`);
}

async function main() {
  const args = process.argv.slice(2);
  const buildAll = args.includes('--all');

  let targets;
  if (buildAll) {
    targets = TARGETS;
  } else {
    targets = [getHostTarget()];
  }

  console.log(`Building Elysium v${PKG_VERSION} for ${targets.length} target(s)...\n`);

  for (const target of targets) {
    const binName = target.includes('windows') ? 'elysium.exe' : 'elysium';
    try {
      cargoBuild(target);
      gzipBinary(target, binName);
    } catch (e) {
      console.error(`  FAILED: ${e.message}`);
    }
    console.log('');
  }

  console.log(`\nDone! Tarballs in: ${OUT_DIR}`);
}

main().catch((e) => {
  console.error('Build failed:', e.message);
  process.exit(1);
});
