import { normalizeArch, normalizePlatform } from "./platform";
import type {
  NormalizedArch,
  NormalizedPlatform,
  TargetDescriptor,
} from "./types";

const TARGET_MATRIX: Record<
  NormalizedPlatform,
  Partial<Record<NormalizedArch, TargetDescriptor>>
> = {
  darwin: {
    arm64: {
      artifact: "curlpit-aarch64-apple-darwin.tar.xz",
      binaryName: "curlpit",
    },
    x64: {
      artifact: "curlpit-x86_64-apple-darwin.tar.xz",
      binaryName: "curlpit",
    },
  },
  linux: {
    x64: {
      artifact: "curlpit-x86_64-unknown-linux-gnu.tar.xz",
      binaryName: "curlpit",
    },
    arm64: {
      artifact: "curlpit-aarch64-unknown-linux-gnu.tar.xz",
      binaryName: "curlpit",
    },
  },
  win32: {
    x64: {
      artifact: "curlpit-x86_64-pc-windows-msvc.zip",
      binaryName: "curlpit.exe",
    },
  },
};

export const targets = TARGET_MATRIX;

export function resolveTarget(
  platform: NormalizedPlatform,
  arch: NormalizedArch,
): TargetDescriptor | undefined {
  return TARGET_MATRIX[platform]?.[arch];
}

export function determineBinaryName(platform: string, arch: string): string | undefined {
  const normalizedPlatform = normalizePlatform(platform);
  const normalizedArch = normalizeArch(arch);
  if (!normalizedPlatform || !normalizedArch) {
    return undefined;
  }
  return resolveTarget(normalizedPlatform, normalizedArch)?.binaryName;
}
