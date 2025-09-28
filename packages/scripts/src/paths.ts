import { join } from "node:path";
import type { NormalizedPlatform } from "./types.ts";

export function defaultBinDir(platform: NormalizedPlatform, homeDir: string) {
  if (platform === "win32") {
    return join(homeDir, "AppData", "Local", "curlpit", "bin");
  }
  return join(homeDir, ".local", "bin");
}
