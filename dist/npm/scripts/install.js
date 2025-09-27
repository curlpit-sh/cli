#!/usr/bin/env node

/**
 * Download and unpack the curlpit binary for the current platform.
 */
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { pipeline } = require('node:stream/promises');
const { Readable } = require('node:stream');
const tar = require('tar');
const AdmZip = require('adm-zip');

const PACKAGE_ROOT = path.resolve(__dirname, '..');
const VENDOR_DIR = path.join(PACKAGE_ROOT, 'vendor');

const TARGETS = {
  darwin: {
    arm64: {
      artifact: 'curlpit-aarch64-apple-darwin.tar.gz',
      binaryName: 'curlpit',
    },
    x64: {
      artifact: 'curlpit-x86_64-apple-darwin.tar.gz',
      binaryName: 'curlpit',
    },
  },
  linux: {
    x64: {
      artifact: 'curlpit-x86_64-unknown-linux-gnu.tar.gz',
      binaryName: 'curlpit',
    },
  },
  win32: {
    x64: {
      artifact: 'curlpit-x86_64-pc-windows-msvc.zip',
      binaryName: 'curlpit.exe',
    },
  },
};

async function main() {
  if (process.env.CURLPIT_SKIP_POSTINSTALL) {
    return;
  }

  const localBinary = process.env.CURLPIT_LOCAL_BINARY;
  if (localBinary) {
    await installFromLocalBinary(localBinary);
    return;
  }

  const platform = process.env.CURLPIT_PLATFORM || process.platform;
  const arch = process.env.CURLPIT_ARCH || process.arch;
  const repo = process.env.CURLPIT_REPOSITORY || 'curlpit-sh/cli';
  const version = process.env.CURLPIT_VERSION || require('../package.json').version;

  const targetInfo = TARGETS[platform]?.[arch];
  if (!targetInfo) {
    console.error(`Unsupported platform/architecture combo: ${platform} ${arch}`);
    console.error('Set CURLPIT_SKIP_POSTINSTALL=1 to skip downloading the binary.');
    process.exit(1);
  }

  const tag = version.startsWith('v') ? version : `v${version}`;
  const baseUrl = process.env.CURLPIT_DOWNLOAD_BASE || 'https://github.com';
  const artifactName = targetInfo.artifact;
  const downloadUrl = `${baseUrl}/${repo}/releases/download/${tag}/${artifactName}`;
  const checksumUrl = `${downloadUrl}.sha256`;

  await fsp.mkdir(VENDOR_DIR, { recursive: true });

  const tempDir = await fsp.mkdtemp(path.join(os.tmpdir(), 'curlpit-download-'));
  const archivePath = path.join(tempDir, artifactName);

  try {
    console.log(`Downloading curlpit binary (${platform}/${arch}) from ${downloadUrl}`);
    await downloadFile(downloadUrl, archivePath);

    if (!process.env.CURLPIT_SKIP_CHECKSUM) {
      await verifyChecksum(archivePath, checksumUrl);
    }

    const extractedPath = await extractArchive(archivePath, tempDir);
    const destination = path.join(VENDOR_DIR, targetInfo.binaryName);
    await fsp.copyFile(extractedPath, destination);

    if (platform !== 'win32') {
      await fsp.chmod(destination, 0o755);
    }

    console.log(`curlpit binary installed to ${destination}`);
  } catch (error) {
    console.error('Failed to install curlpit binary:', error.message ?? error);
    process.exit(1);
  } finally {
    await cleanupTemp(tempDir);
  }
}

async function installFromLocalBinary(binaryPath) {
  const resolved = path.resolve(binaryPath);
  const exists = await fsp
    .access(resolved, fs.constants.X_OK)
    .then(() => true)
    .catch(() => false);
  if (!exists) {
    throw new Error(`Local binary not found or not executable: ${resolved}`);
  }

  await fsp.mkdir(VENDOR_DIR, { recursive: true });
  const destination = path.join(
    VENDOR_DIR,
    process.platform === 'win32' ? 'curlpit.exe' : 'curlpit'
  );
  await fsp.copyFile(resolved, destination);
  if (process.platform !== 'win32') {
    await fsp.chmod(destination, 0o755);
  }
  console.log(`curlpit binary installed from local path ${resolved}`);
}

async function downloadFile(url, destination) {
  const response = await fetch(url);
  if (!response.ok || !response.body) {
    throw new Error(`Download failed with status ${response.status} ${response.statusText}`);
  }
  await pipeline(Readable.fromWeb(response.body), fs.createWriteStream(destination));
}

async function verifyChecksum(filePath, checksumUrl) {
  const response = await fetch(checksumUrl);
  if (!response.ok || !response.body) {
    throw new Error(`Checksum download failed (${response.status} ${response.statusText}). Set CURLPIT_SKIP_CHECKSUM=1 to override.`);
  }

  const checksumText = await response.text();
  const expected = checksumText.trim().split(/\s+/)[0];
  if (!expected) {
    throw new Error('Checksum file was empty.');
  }

  const actual = await computeSha256(filePath);

  if (actual !== expected) {
    throw new Error(`Checksum mismatch: expected ${expected}, got ${actual}`);
  }
}

async function computeSha256(filePath) {
  const hash = crypto.createHash('sha256');
  return new Promise((resolve, reject) => {
    const stream = fs.createReadStream(filePath);
    stream.on('error', reject);
    stream.on('data', (chunk) => hash.update(chunk));
    stream.on('close', () => resolve(hash.digest('hex')));
  });
}

async function extractArchive(archivePath, tempDir) {
  if (archivePath.endsWith('.tar.gz')) {
    await tar.x({ file: archivePath, cwd: tempDir });
  } else if (archivePath.endsWith('.zip')) {
    const zip = new AdmZip(archivePath);
    zip.extractAllTo(tempDir, true);
  } else {
    throw new Error(`Unsupported archive format: ${archivePath}`);
  }

  const entries = await fsp.readdir(tempDir);
  for (const entry of entries) {
    if (entry === path.basename(archivePath)) {
      continue;
    }
    if (!entry.startsWith('curlpit')) {
      continue;
    }
    const candidate = path.join(tempDir, entry);
    const stat = await fsp.stat(candidate);
    if (stat.isFile()) {
      return candidate;
    }
  }

  throw new Error('Extracted archive did not contain a curlpit binary.');
}

async function cleanupTemp(dir) {
  try {
    await fsp.rm(dir, { recursive: true, force: true });
  } catch (error) {
    // Ignore cleanup errors
  }
}

main().catch((error) => {
  console.error('Unexpected error during curlpit installation:', error);
  process.exit(1);
});
