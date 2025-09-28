import { spawn } from "node:child_process";
import { readdir } from "node:fs/promises";
import { join } from "node:path";
import type { ExtractOptions } from "./types";

export async function extractArchive({
  archivePath,
  tempDir,
  platform,
  tarPath,
}: ExtractOptions): Promise<string> {
  if (archivePath.endsWith(".tar.gz")) {
    await runCommand(tarPath ?? "tar", ["-xzf", archivePath, "-C", tempDir]);
  } else if (archivePath.endsWith(".tar.xz")) {
    await runCommand(tarPath ?? "tar", ["-xJf", archivePath, "-C", tempDir]);
  } else if (archivePath.endsWith(".zip")) {
    if (platform === "win32") {
      await runCommand("powershell", [
        "-NoProfile",
        "-Command",
        `Expand-Archive -Path \"${archivePath}\" -DestinationPath \"${tempDir}\" -Force`,
      ]);
    } else {
      await runCommand("unzip", [archivePath, "-d", tempDir]);
    }
  } else {
    throw new Error(`Unsupported archive format: ${archivePath}`);
  }

  const binaryPath = await locateBinary(tempDir);
  if (!binaryPath) {
    throw new Error("Archive did not contain a curlpit binary");
  }
  return binaryPath;
}

async function locateBinary(dir: string): Promise<string | undefined> {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const candidate = join(dir, entry.name);
    if (entry.isFile() && entry.name.startsWith("curlpit")) {
      return candidate;
    }
    if (entry.isDirectory()) {
      const nested = await locateBinary(candidate);
      if (nested) {
        return nested;
      }
    }
  }
  return undefined;
}

export async function runCommand(command: string, args: string[]) {
  await new Promise<void>((resolve, reject) => {
    const child = spawn(command, args, { stdio: "inherit" });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${command} exited with code ${code}`));
      }
    });
  });
}
