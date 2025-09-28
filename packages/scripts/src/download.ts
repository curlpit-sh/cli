import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import type { DownloadOptions } from "./types";

export async function downloadArtifact({
  artifactUrl,
  destination,
}: DownloadOptions) {
  const response = await fetch(artifactUrl);
  if (!response.ok || !response.body) {
    throw new Error(
      `Failed to download ${artifactUrl} (${response.status} ${response.statusText})`,
    );
  }

  const body: unknown = response.body;
  let stream: NodeJS.ReadableStream;
  if (body && typeof (body as { getReader?: () => unknown }).getReader === "function") {
    stream = Readable.fromWeb(body as any);
  } else if (body) {
    stream = body as NodeJS.ReadableStream;
  } else {
    throw new Error("Download response body was not readable");
  }

  await pipeline(stream, createWriteStream(destination));
}

export async function fetchChecksumText(url: string) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to download checksum (${response.status} ${response.statusText})`);
  }
  const text = (await response.text()).trim();
  if (!text) {
    throw new Error("Checksum file was empty");
  }
  return text.split(/\s+/)[0];
}

export async function verifyChecksum(filePath: string, expected: string) {
  const hash = createHash("sha256");
  await pipeline(createReadStream(filePath), hash);
  const actual = hash.digest("hex");
  if (actual !== expected) {
    throw new Error(`Checksum mismatch: expected ${expected}, got ${actual}`);
  }
}
