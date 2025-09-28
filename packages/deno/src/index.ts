// biome-ignore lint/suspicious/noTsIgnore: Deno
// @ts-ignore Deno
/// <reference lib="deno.ns" />

import { join } from "node:path";
import {
  cleanup,
  createEnvReader,
  createTempDir,
  defaultBinDir,
  downloadArtifact,
  ensureDir,
  ensureExecutable,
  extractArchive,
  fetchChecksumText,
  guessHomeDir,
  planRelease,
  verifyChecksum,
} from "@curlpit/scripts";

const env = createEnvReader();
const platform = env("CURLPIT_PLATFORM") ?? Deno.build.os;
const arch = env("CURLPIT_ARCH") ?? Deno.build.arch;
const repo = env("CURLPIT_REPOSITORY") ?? "curlpit-sh/cli";
const version = env("CURLPIT_VERSION") ?? "v0.3.0";
const baseUrl = env("CURLPIT_DOWNLOAD_BASE") ?? "https://github.com";
const skipChecksum = Boolean(env("CURLPIT_SKIP_CHECKSUM"));
const silent = Boolean(env("CURLPIT_SILENT"));
const tarPath = env("TAR_PATH");

const plan = planRelease({ platform, arch, version, baseUrl, repo });
const home = guessHomeDir(env);
const binDir = env("CURLPIT_BIN_DIR") ?? defaultBinDir(plan.platform, home);
const binaryPath = join(binDir, plan.target.binaryName);

const tempDir = await createTempDir("curlpit-deno-");
const archivePath = join(tempDir, plan.target.artifact);

try {
  console.log(`Downloading ${plan.artifactUrl}`);
  await downloadArtifact({ artifactUrl: plan.artifactUrl, destination: archivePath });

  if (!skipChecksum) {
    console.log("Verifying checksum");
    const expectedChecksum = await fetchChecksumText(plan.checksumUrl);
    await verifyChecksum(archivePath, expectedChecksum);
  } else {
    console.warn("Skipping checksum verification (CURLPIT_SKIP_CHECKSUM set)");
  }

  console.log("Extracting archive");
  const extractedBinary = await extractArchive({
    archivePath,
    tempDir,
    platform: plan.platform,
    tarPath: tarPath ?? undefined,
  });

  await ensureBinary(extractedBinary, binaryPath, plan.platform !== "win32");

  console.log(`curlpit installed to ${binaryPath}`);
  if (!silent) {
    console.log(`Add '${binDir}' to your PATH if it isn't already.`);
  }
} catch (error) {
  console.error("curlpit installation failed:", error);
  Deno.exit(1);
} finally {
  await cleanup(tempDir);
}

async function ensureBinary(source: string, destination: string, makeExecutable: boolean) {
  await ensureDir(binDir);
  await ensureExecutable({
    sourcePath: source,
    destinationPath: destination,
    makeExecutable,
  });
}
