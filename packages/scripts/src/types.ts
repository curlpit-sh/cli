export type NormalizedPlatform = "darwin" | "linux" | "win32";
export type NormalizedArch = "x64" | "arm64";

export interface TargetDescriptor {
  artifact: string;
  binaryName: string;
}

export interface InstallContext {
  platform: string;
  arch: string;
  version: string;
  baseUrl: string;
  repo: string;
}

export interface ReleasePlan {
  platform: NormalizedPlatform;
  arch: NormalizedArch;
  target: TargetDescriptor;
  tag: string;
  artifactUrl: string;
  checksumUrl: string;
}

export interface DownloadOptions {
  artifactUrl: string;
  destination: string;
}

export interface ExtractOptions {
  archivePath: string;
  tempDir: string;
  platform: NormalizedPlatform;
  tarPath?: string;
}

export interface EnsureBinaryOptions {
  sourcePath: string;
  destinationPath: string;
  makeExecutable: boolean;
}

export type EnvReader = (key: string) => string | undefined;
