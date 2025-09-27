#!/usr/bin/env node

const path = require('node:path');
const fs = require('node:fs');
const { spawn } = require('node:child_process');

function main() {
  const vendorDir = path.resolve(__dirname, '..', 'vendor');
  const binaryName = process.platform === 'win32' ? 'curlpit.exe' : 'curlpit';
  const binaryPath = path.join(vendorDir, binaryName);

  if (!fs.existsSync(binaryPath)) {
    console.error(
      'curlpit binary is missing. Try reinstalling the package or run "npm rebuild curlpit".'
    );
    process.exit(1);
  }

  const child = spawn(binaryPath, process.argv.slice(2), {
    stdio: 'inherit',
  });

  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 0);
  });

  child.on('error', (error) => {
    console.error('Failed to start curlpit binary:', error);
    process.exit(1);
  });
}

main();
