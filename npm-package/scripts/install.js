#!/usr/bin/env node

/**
 * Post-install script for elysium-lang.
 *
 * Downloads the prebuilt elysium and epm binaries for the current platform
 * from GitHub Releases. Falls back to building from source if no prebuilt
 * binary is available.
 *
 * This script has zero external dependencies — only Node.js built-ins.
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const https = require('https');
const zlib = require('zlib');

const PKG_VERSION = require('../package.json').version;
const BIN_DIR = path.resolve(__dirname, '..', 'bin');

/**
 * Map process.platform + arch → Rust target triple.
 */
function getTarget() {
  const os = process.platform;
  const arch = process.arch;
  let osName, archName;

  if (os === 'darwin') osName = 'apple-darwin';
  else if (os === 'linux') osName = 'unknown-linux-gnu';
  else if (os === 'win32') osName = 'pc-windows-msvc';
  else throw new Error(`Unsupported platform: ${os}`);

  if (arch === 'x64') archName = 'x86_64';
  else if (arch === 'arm64') archName = 'aarch64';
  else throw new Error(`Unsupported architecture: ${arch}`);

  return `${archName}-${osName}`;
}

function download(url, destPath) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(destPath);
    const req = https.get(url, (response) => {
      // Follow redirects
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        file.close();
        fs.unlinkSync(destPath);
        download(response.headers.location, destPath).then(resolve).catch(reject);
        return;
      }
      if (response.statusCode !== 200) {
        file.close();
        fs.unlinkSync(destPath);
        reject(new Error(`HTTP ${response.statusCode}: ${url}`));
        return;
      }
      response.pipe(file);
      file.on('finish', () => { file.close(); resolve(); });
    });
    req.on('error', (err) => {
      file.close();
      fs.unlinkSync(destPath, () => {});
      reject(err);
    });
  });
}

function extractGzip(inputPath, outputPath) {
  return new Promise((resolve, reject) => {
    const input = fs.createReadStream(inputPath);
    const output = fs.createWriteStream(outputPath);
    const gunzip = zlib.createGunzip();
    input.pipe(gunzip).pipe(output);
    output.on('finish', resolve);
    output.on('error', reject);
  });
}

/**
 * Download a gzipped binary from GitHub Releases and install it into bin/.
 *
 * @param {string} label  — "elysium" or "epm"
 * @param {string} target — Rust target triple
 */
async function downloadBinary(label, target) {
  const repo = 'imstevetran/elysium';
  const tag = `v${PKG_VERSION}`;
  const ext = target.includes('windows') ? '.exe' : '';
  const binFilename = `${label}${ext}`;
  const archiveName = `${label}-${PKG_VERSION}-${target}.gz`;
  const url = `https://github.com/${repo}/releases/download/${tag}/${archiveName}`;
  const tmpDir = fs.mkdtempSync(path.join(require('os').tmpdir(), `elysium-install-${label}-`));
  const gzPath = path.join(tmpDir, archiveName);

  console.log(`Downloading ${label}: ${url}`);
  await download(url, gzPath);

  // Decompress gzip to get the binary
  const binPath = path.join(tmpDir, binFilename);
  await extractGzip(gzPath, binPath);

  // Copy to bin dir, preserving the original name (elysium or epm)
  fs.mkdirSync(BIN_DIR, { recursive: true });
  const dstPath = path.join(BIN_DIR, binFilename);
  fs.copyFileSync(binPath, dstPath);
  if (!target.includes('windows')) {
    fs.chmodSync(dstPath, 0o755);
  }
  console.log(`  installed: ${dstPath}`);

  // Cleanup
  fs.rmSync(tmpDir, { recursive: true, force: true });
}

function buildFromSource(label) {
  const binName = process.platform === 'win32' ? `${label}.exe` : label;
  const rootDir = path.resolve(__dirname, '..', '..');

  console.log(`No prebuilt binary for ${label}. Building from source...`);
  console.log('This requires Rust to be installed (https://rustup.rs).\n');

  try {
    execSync(`cargo build --release --bin ${label}`, { cwd: rootDir, stdio: 'inherit' });
    const srcBin = path.join(rootDir, 'target', 'release', binName);
    const dstBin = path.join(BIN_DIR, binName);
    fs.mkdirSync(BIN_DIR, { recursive: true });
    fs.copyFileSync(srcBin, dstBin);
    if (process.platform !== 'win32') fs.chmodSync(dstBin, 0o755);
    console.log(`Built and installed: ${dstBin}`);
  } catch (e) {
    console.error(`Build failed for ${label}. Install Rust from https://rustup.rs`);
    process.exit(1);
  }
}

async function installBinary(label) {
  try {
    const target = getTarget();
    await downloadBinary(label, target);
  } catch (e) {
    console.log(`${label} prebuilt binary unavailable: ${e.message}`);
    buildFromSource(label);
  }
}

async function main() {
  try {
    fs.mkdirSync(BIN_DIR, { recursive: true });

    // Install both binaries
    await installBinary('elysium');
    await installBinary('epm');

    console.log('\nElysium installed! Commands available:');
    console.log('  npx elysium --help    (or: npx ely --help)');
    console.log('  npx epm --help');
  } catch (e) {
    console.error('Installation failed:', e.message);
    process.exit(1);
  }
}

main();
