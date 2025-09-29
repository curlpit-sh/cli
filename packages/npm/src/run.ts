#!/usr/bin/env node

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, resolve as resolvePath } from "node:path";
import { fileURLToPath } from "node:url";
import { createEnvReader, determineBinaryName } from "@curlpit/scripts";

function main() {
  const envReader = createEnvReader();
  const platform = envReader("CURLPIT_PLATFORM") ?? process.platform;
  const arch = envReader("CURLPIT_ARCH") ?? process.arch;

  const binaryName =
    determineBinaryName(platform, arch) ??
    (platform === "win32" ? "curlpit.exe" : "curlpit");

  const moduleDir = dirname(fileURLToPath(import.meta.url));
  const packageRoot = resolvePath(moduleDir, "..");
  const binDir = envReader("CURLPIT_BIN_DIR") ?? moduleDir;

  const vendorDir = join(packageRoot, "vendor");
  const candidatePaths = [
    join(binDir, binaryName),
    join(vendorDir, binaryName),
  ];
  const binaryPath = candidatePaths.find((candidate) => existsSync(candidate));

  if (!binaryPath) {
    console.error(
      `curlpit binary is missing. Expected at ${candidatePaths.join(" or ")}.
Try reinstalling the package or run "npm rebuild curlpit".`,
    );
    process.exit(1);
  }

  const runtime = detectRuntime();
  const childEnv = { ...process.env } as NodeJS.ProcessEnv;
  if (!childEnv.CURLPIT_RUNTIME) {
    childEnv.CURLPIT_RUNTIME = runtime;
  }

  const child = spawn(binaryPath, process.argv.slice(2), {
    stdio: "inherit",
    env: childEnv,
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 0);
  });

  child.on("error", (error) => {
    console.error("Failed to start curlpit binary:", error);
    process.exit(1);
  });
}

function detectRuntime() {
  if (typeof (globalThis as { Deno?: unknown }).Deno !== "undefined") {
    return "deno";
  }
  if (typeof process.versions?.bun !== "undefined") {
    return "bun";
  }
  return "node";
}

main();
