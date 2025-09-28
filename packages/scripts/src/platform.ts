import type { NormalizedArch, NormalizedPlatform } from "./types.ts";

const PLATFORM_ALIASES: Record<string, NormalizedPlatform> = {
  darwin: "darwin",
  macos: "darwin",
  macosx: "darwin",
  osx: "darwin",
  linux: "linux",
  win32: "win32",
  windows: "win32",
};

const ARCH_ALIASES: Record<string, NormalizedArch> = {
  x64: "x64",
  amd64: "x64",
  "x86_64": "x64",
  arm64: "arm64",
  aarch64: "arm64",
};

export function normalizePlatform(value: string | undefined): NormalizedPlatform | undefined {
  if (!value) return undefined;
  return PLATFORM_ALIASES[value.toLowerCase()];
}

export function normalizeArch(value: string | undefined): NormalizedArch | undefined {
  if (!value) return undefined;
  return ARCH_ALIASES[value.toLowerCase()];
}

export function normalizePlatformOrThrow(value: string): NormalizedPlatform {
  const normalized = normalizePlatform(value);
  if (!normalized) {
    throw new Error(`Unsupported platform: ${value}`);
  }
  return normalized;
}

export function normalizeArchOrThrow(value: string): NormalizedArch {
  const normalized = normalizeArch(value);
  if (!normalized) {
    throw new Error(`Unsupported architecture: ${value}`);
  }
  return normalized;
}

export function normalizePlatformLoose(value: string | undefined) {
  return normalizePlatform(value);
}

export function normalizeArchLoose(value: string | undefined) {
  return normalizeArch(value);
}

export function isWindowsPlatform(value: string): boolean {
  return normalizePlatform(value) === "win32";
}
