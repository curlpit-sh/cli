// deno-lint-ignore-file no-explicit-any
/**
 * curlpit installer for Deno.
 *
 * Usage:
 *   deno run -A https://raw.githubusercontent.com/curlpit-sh/cli/refs/heads/main/dist/deno/install.ts
 *
 * Permissions required: --allow-env --allow-net --allow-run --allow-read --allow-write
 */

import { join } from "@std/path";

const TARGETS: Record<
  string,
  Record<string, { artifact: string; binaryName: string }>
> = {
  darwin: {
    aarch64: {
      artifact: "curlpit-aarch64-apple-darwin.tar.xz",
      binaryName: "curlpit",
    },
    x86_64: {
      artifact: "curlpit-x86_64-apple-darwin.tar.xz",
      binaryName: "curlpit",
    },
  },
  linux: {
    x86_64: {
      artifact: "curlpit-x86_64-unknown-linux-gnu.tar.xz",
      binaryName: "curlpit",
    },
    aarch64: {
      artifact: "curlpit-aarch64-unknown-linux-gnu.tar.xz",
      binaryName: "curlpit",
    },
  },
  windows: {
    x86_64: {
      artifact: "curlpit-x86_64-pc-windows-msvc.zip",
      binaryName: "curlpit.exe",
    },
  },
};

const env = Deno.env;
const platform = env.get("CURLPIT_PLATFORM") ?? Deno.build.os;
const arch = env.get("CURLPIT_ARCH") ?? Deno.build.arch;
const repo = env.get("CURLPIT_REPOSITORY") ?? "curlpit-sh/cli";
const version = env.get("CURLPIT_VERSION") ?? "v0.2.7";
const baseUrl = env.get("CURLPIT_DOWNLOAD_BASE") ?? "https://github.com";

const target = TARGETS[platform]?.[arch];
if (!target) {
  console.error(`Unsupported platform/architecture: ${platform} ${arch}`);
  console.error(
    "Override CURLPIT_PLATFORM/CURLPIT_ARCH or install manually from releases.",
  );
  Deno.exit(1);
}

const tag = version.startsWith("v") ? version : `v${version}`;
const artifactUrl = `${baseUrl}/${repo}/releases/download/${tag}/${target.artifact}`;
const checksumUrl = `${artifactUrl}.sha256`;

const home = env.get("HOME") ?? env.get("USERPROFILE") ?? Deno.cwd();
const defaultBinDir =
  platform === "windows"
    ? join(home, "AppData", "Local", "curlpit", "bin")
    : join(home, ".local", "bin");
const binDir = env.get("CURLPIT_BIN_DIR") ?? defaultBinDir;

const binaryPath = join(binDir, target.binaryName);

const tempDir = await Deno.makeTempDir({ prefix: "curlpit-deno-" });
const archivePath = join(tempDir, target.artifact);

try {
  console.log(`Downloading ${artifactUrl}`);
  await downloadFile(artifactUrl, archivePath);

  if (!env.get("CURLPIT_SKIP_CHECKSUM")) {
    console.log("Verifying checksum");
    await verifyChecksum(archivePath, checksumUrl);
  } else {
    console.warn("Skipping checksum verification (CURLPIT_SKIP_CHECKSUM set)");
  }

  console.log("Extracting archive");
  const extractedBinary = await extractArchive(
    archivePath,
    tempDir,
    platform === "windows",
  );

  await ensureDir(binDir);
  await Deno.copyFile(extractedBinary, binaryPath);
  if (platform !== "windows") {
    await Deno.chmod(binaryPath, 0o755);
  }

  console.log(`curlpit installed to ${binaryPath}`);
  if (!Deno.env.get("CURLPIT_SILENT")) {
    console.log(`Add '${binDir}' to your PATH if it isn't already.`);
  }
} catch (error) {
  console.error("curlpit installation failed:", error);
  Deno.exit(1);
} finally {
  try {
    await Deno.remove(tempDir, { recursive: true });
  } catch (_) {
    /* ignore */
  }
}

async function downloadFile(url: string, destination: string) {
  const response = await fetch(url);
  if (!response.ok || !response.body) {
    throw new Error(
      `Failed to download ${url} (${response.status} ${response.statusText})`,
    );
  }

  const file = await Deno.open(destination, {
    write: true,
    create: true,
    truncate: true,
  });
  try {
    await response.body.pipeTo(file.writable);
  } finally {
    file.close();
  }
}

async function verifyChecksum(filePath: string, checksumUrl: string) {
  const response = await fetch(checksumUrl);
  if (!response.ok) {
    throw new Error(
      `Failed to download checksum (${response.status} ${response.statusText})`,
    );
  }
  const expected = (await response.text()).trim().split(/\s+/)[0];
  if (!expected) {
    throw new Error("Checksum file was empty");
  }

  const data = await Deno.readFile(filePath);
  const hashBuffer = await crypto.subtle.digest("SHA-256", data);
  const actual = bufferToHex(hashBuffer);
  if (actual !== expected) {
    throw new Error(`Checksum mismatch: expected ${expected}, got ${actual}`);
  }
}

async function extractArchive(
  archivePath: string,
  temp: string,
  isWindows: boolean,
): Promise<string> {
  if (archivePath.endsWith(".tar.gz")) {
    await runTar(["-xzf", archivePath, "-C", temp]);
  } else if (archivePath.endsWith(".tar.xz")) {
    await runTar(["-xJf", archivePath, "-C", temp]);
  } else if (archivePath.endsWith(".zip")) {
    if (isWindows) {
      const cmd = new Deno.Command("powershell", {
        args: [
          "-NoProfile",
          "-Command",
          `Expand-Archive -Path \"${archivePath}\" -DestinationPath \"${temp}\" -Force`,
        ],
      });
      const { success, stderr } = await cmd.output();
      if (!success) {
        throw new Error(new TextDecoder().decode(stderr));
      }
    } else {
      const cmd = new Deno.Command("unzip", {
        args: [archivePath, "-d", temp],
      });
      const { success, stderr } = await cmd.output();
      if (!success) {
        throw new Error(new TextDecoder().decode(stderr));
      }
    }
  } else {
    throw new Error(`Unsupported archive format: ${archivePath}`);
  }

  for await (const entry of Deno.readDir(temp)) {
    if (entry.isFile && entry.name.startsWith("curlpit")) {
      return join(temp, entry.name);
    }
  }
  throw new Error("Archive did not contain a curlpit binary");
}

async function ensureDir(dir: string) {
  await Deno.mkdir(dir, { recursive: true });
}

function bufferToHex(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

async function runTar(args: string[]) {
  const tarPath = Deno.env.get("TAR_PATH") ?? "tar";
  const cmd = new Deno.Command(tarPath, { args });
  const { success, stderr } = await cmd.output();
  if (!success) {
    throw new Error(new TextDecoder().decode(stderr));
  }
}
