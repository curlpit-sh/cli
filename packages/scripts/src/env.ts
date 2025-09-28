import { tmpdir } from "node:os";
import type { EnvReader } from "./types";

const maybeDenoEnv = (() => {
  try {
    const deno = (globalThis as { Deno?: { env?: { get(name: string): string | undefined } } }).Deno;
    return deno?.env;
  } catch {
    return undefined;
  }
})();

export function createEnvReader(): EnvReader {
  return (key: string) => {
    if (typeof process !== "undefined" && process.env && key in process.env) {
      return process.env[key];
    }
    return maybeDenoEnv?.get?.(key);
  };
}

export function guessHomeDir(env: EnvReader): string {
  return env("HOME") ?? env("USERPROFILE") ?? env("HOMEPATH") ?? tmpdir();
}

export function inferPlatform(env: EnvReader): string {
  if (typeof process !== "undefined" && process.platform) {
    return process.platform;
  }
  return env("CURLPIT_PLATFORM") ?? "";
}

export function inferArch(env: EnvReader): string {
  if (typeof process !== "undefined" && process.arch) {
    return process.arch;
  }
  return env("CURLPIT_ARCH") ?? "";
}
