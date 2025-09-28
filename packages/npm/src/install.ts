#!/usr/bin/env node

import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  cleanup,
  createEnvReader,
  createTempDir,
  installLocalBinary,
  downloadArtifact,
  ensureExecutable,
  extractArchive,
  fetchChecksumText,
  planRelease,
  verifyChecksum,
} from "@curlpit/scripts";
import packageJson from "../package.json" with { type: "json" };

async function main() {
  const env = createEnvReader();
  const moduleDir = dirname(fileURLToPath(import.meta.url));

  if (env("CURLPIT_SKIP_POSTINSTALL")) {
    return;
  }

  const platform = env("CURLPIT_PLATFORM") ?? process.platform;
  const arch = env("CURLPIT_ARCH") ?? process.arch;

  const repo = env("CURLPIT_REPOSITORY") ?? "curlpit-sh/cli";
  const version = env("CURLPIT_VERSION") ?? packageJson.version;
  const baseUrl = env("CURLPIT_DOWNLOAD_BASE") ?? "https://github.com";
  const skipChecksum = Boolean(env("CURLPIT_SKIP_CHECKSUM"));
  const tarPath = env("TAR_PATH");

  const plan = planRelease({ platform, arch, version, baseUrl, repo });
  const binDir = env("CURLPIT_BIN_DIR") ?? moduleDir;
  const binaryName = plan.target.binaryName;
  const binaryDestination = join(binDir, binaryName);

  const localBinary = env("CURLPIT_LOCAL_BINARY");
  if (localBinary) {
    await installLocalBinary(
      localBinary,
      binDir,
      binaryName,
      plan.platform !== "win32",
    );
    console.log(`curlpit binary installed to ${binaryDestination}`);
    return;
  }

  const tempDir = await createTempDir("curlpit-npm-");
  const archivePath = join(tempDir, plan.target.artifact);

  try {
    console.log(
      `Downloading curlpit binary (${platform}/${arch}) from ${plan.artifactUrl}`,
    );
    await downloadArtifact({
      artifactUrl: plan.artifactUrl,
      destination: archivePath,
    });

    if (!skipChecksum) {
      const expectedChecksum = await fetchChecksumText(plan.checksumUrl);
      await verifyChecksum(archivePath, expectedChecksum);
    } else {
      console.warn(
        "Skipping checksum verification (CURLPIT_SKIP_CHECKSUM set)",
      );
    }

    const extractedBinary = await extractArchive({
      archivePath,
      tempDir,
      platform: plan.platform,
      tarPath: tarPath ?? undefined,
    });

    await ensureExecutable({
      sourcePath: extractedBinary,
      destinationPath: binaryDestination,
      makeExecutable: plan.platform !== "win32",
    });

    console.log(`curlpit binary installed to ${binaryDestination}`);
  } catch (error) {
    console.error(
      "Failed to install curlpit binary:",
      error instanceof Error ? error.message : error,
    );
    process.exit(1);
  } finally {
    await cleanup(tempDir);
  }
}

main().catch((error) => {
  console.error(
    "Unexpected error during curlpit installation:",
    error instanceof Error ? (error.stack ?? error.message) : error,
  );
  process.exit(1);
});
