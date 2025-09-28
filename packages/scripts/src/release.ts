import { normalizeArchOrThrow, normalizePlatformOrThrow } from "./platform";
import { resolveTarget } from "./targets";
import type { InstallContext, ReleasePlan } from "./types";

export function planRelease(input: InstallContext): ReleasePlan {
  const platform = normalizePlatformOrThrow(input.platform);
  const arch = normalizeArchOrThrow(input.arch);
  const target = resolveTarget(platform, arch);
  if (!target) {
    throw new Error(
      `Unsupported platform/architecture combination: ${input.platform}/${input.arch}`,
    );
  }

  const tag = input.version.startsWith("v") ? input.version : `v${input.version}`;
  const baseUrl = input.baseUrl.replace(/\/$/, "");
  const artifactUrl = `${baseUrl}/${input.repo}/releases/download/${tag}/${target.artifact}`;
  const checksumUrl = `${artifactUrl}.sha256`;

  return {
    platform,
    arch,
    target,
    tag,
    artifactUrl,
    checksumUrl,
  };
}
