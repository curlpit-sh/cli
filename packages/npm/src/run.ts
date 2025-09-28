#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join, resolve as resolvePath } from 'node:path';
import { determineBinaryName } from '@curlpit/scripts';

function main() {
  const vendorDir = resolvePath(__dirname, '..', 'vendor');
  const binaryName = determineBinaryName(process.platform, process.arch) ??
    (process.platform === 'win32' ? 'curlpit.exe' : 'curlpit');
  const binaryPath = join(vendorDir, binaryName);

  if (!existsSync(binaryPath)) {
    console.error(
      'curlpit binary is missing. Try reinstalling the package or run "npm rebuild curlpit".',
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
