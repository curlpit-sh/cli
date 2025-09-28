import { access, chmod, copyFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import { constants as fsConstants } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import type { EnsureBinaryOptions } from "./types";

export async function createTempDir(prefix = "curlpit-") {
  return mkdtemp(join(tmpdir(), prefix));
}

export async function ensureDir(path: string) {
  await mkdir(path, { recursive: true });
}

export async function ensureExecutable({
  sourcePath,
  destinationPath,
  makeExecutable,
}: EnsureBinaryOptions) {
  await ensureDir(dirname(destinationPath));
  await copyFile(sourcePath, destinationPath);
  if (makeExecutable) {
    await chmod(destinationPath, 0o755);
  }
}

export async function cleanup(path: string) {
  await rm(path, { recursive: true, force: true });
}

export async function isExecutable(path: string) {
  try {
    await access(path, fsConstants.X_OK);
    return true;
  } catch {
    return false;
  }
}

export async function installLocalBinary(
  sourcePath: string,
  destinationDir: string,
  binaryName: string,
  makeExecutable: boolean,
) {
  const resolved = resolve(sourcePath);
  const exists = await isExecutable(resolved);
  if (!exists) {
    throw new Error(`Local binary not found or not executable: ${resolved}`);
  }

  await ensureDir(destinationDir);
  const destinationPath = join(destinationDir, binaryName);
  await copyFile(resolved, destinationPath);
  if (makeExecutable) {
    await chmod(destinationPath, 0o755);
  }
  return destinationPath;
}
