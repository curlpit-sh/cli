#!/usr/bin/env bun

import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

interface Descriptor {
  path: string;
  updates: Update[];
}

interface Update {
  type: "json" | "text";
  jsonPointer?: string;
  regex?: RegExp;
  replacer?: (match: string, ...groups: string[]) => string;
}

if (process.argv.length !== 3) {
  console.error("usage: bun bump-version.ts <new-version>");
  process.exit(1);
}

const newVersion = process.argv[2].replace(/^v/, "");
if (!/^\d+\.\d+\.\d+(-[A-Za-z0-9-.]+)?$/.test(newVersion)) {
  console.error(`invalid semver version: ${newVersion}`);
  process.exit(1);
}

const files: Descriptor[] = [
  {
    path: "Cargo.toml",
    updates: [
      {
        type: "text",
        regex: /^version = "[^"]+"/m,
        replacer: () => `version = "${newVersion}"`,
      },
    ],
  },
  {
    path: "Cargo.lock",
    updates: [
      {
        type: "text",
        regex: /(name = "curlpit"\nversion = ")([^"]+)(")/m,
        replacer: (_match, pre, _version, post) => `${pre}${newVersion}${post}`,
      },
      {
        type: "text",
        regex: /(name = "curlpit-wasm"\nversion = ")([^"]+)(")/m,
        replacer: (_match, pre, _version, post) => `${pre}${newVersion}${post}`,
      },
    ],
  },
  {
    path: "packages/npm/package.json",
    updates: [{ type: "json", jsonPointer: "/version" }],
  },
  {
    path: "packages/deno/package.json",
    updates: [{ type: "json", jsonPointer: "/version" }],
  },
  {
    path: "packages/vscode-extension/package.json",
    updates: [{ type: "json", jsonPointer: "/version" }],
  },
  {
    path: "packages/deno/deno.json",
    updates: [{ type: "json", jsonPointer: "/version" }],
  },
  {
    path: "packages/scripts/package.json",
    updates: [{ type: "json", jsonPointer: "/version" }],
  },
  {
    path: "packages/deno/src/index.ts",
    updates: [
      {
        type: "text",
        regex: /(const version = env\("CURLPIT_VERSION"\) \?\? ")v?([^"]+)(")/,
        replacer: (_match, pre, _current, post) =>
          `${pre}v${newVersion}${post}`,
      },
    ],
  },
  {
    path: "packages/brew/curlpit.rb",
    updates: [
      {
        type: "text",
        regex: /(version ")([^"]+)(")/,
        replacer: (_match, pre, _version, post) => `${pre}${newVersion}${post}`,
      },
      {
        type: "text",
        regex:
          /(url "https:\/\/github\.com\/curlpit-sh\/cli\/archive\/refs\/tags\/v)[^"@]+(\.tar\.gz")/,
        replacer: (_match, pre, suffix) => `${pre}${newVersion}${suffix}`,
      },
    ],
  },
  {
    path: "packages/brew/README.md",
    updates: [
      {
        type: "text",
        regex: /(shasum -a 256 curlpit-)[0-9.]+(\.tar\.gz)/,
        replacer: (_match, pre, suffix) => `${pre}${newVersion}${suffix}`,
      },
    ],
  },
  {
    path: "packages/zed-extension/extension.toml",
    updates: [
      {
        type: "text",
        regex: /^version = "[^"]+"/m,
        replacer: () => `version = "${newVersion}"`,
      },
    ],
  },
  {
    path: "packages/www/curlpit-wasm/Cargo.toml",
    updates: [
      {
        type: "text",
        regex: /^version = "[^"]+"/m,
        replacer: () => `version = "${newVersion}"`,
      },
    ],
  },
  {
    path: "packages/www/curlpit-wasm/Cargo.lock",
    updates: [
      {
        type: "text",
        regex: /(name = "curlpit-wasm"\nversion = ")([^"]+)(")/m,
        replacer: (_match, pre, _version, post) => `${pre}${newVersion}${post}`,
      },
    ],
  },
  {
    path: "packages/package.json",
    updates: [{ type: "json", jsonPointer: "/version" }],
  },
];

async function updateFile(descriptor: Descriptor) {
  const fullPath = resolve(descriptor.path);
  let content = await readFile(fullPath, "utf8");
  const original = content;

  for (const update of descriptor.updates) {
    switch (update.type) {
      case "json":
        content = update.jsonPointer
          ? applyJson(content, update.jsonPointer)
          : content;
        break;
      case "text":
        if (!update.regex || !update.replacer) {
          throw new Error(
            `text update for ${descriptor.path} missing regex/replacer`,
          );
        }
        if (!update.regex.test(content)) {
          console.warn(
            `warning: pattern ${update.regex} not found in ${descriptor.path}`,
          );
        }
        content = content.replace(update.regex, update.replacer);
        break;
    }
  }

  if (content !== original) {
    await writeFile(fullPath, content, "utf8");
    console.log(`updated ${descriptor.path}`);
  } else {
    console.log(`no changes for ${descriptor.path}`);
  }
}

function applyJson(content: string, pointer: string): string {
  const data = JSON.parse(content);
  setByPointer(data, pointer, newVersion);
  return `${JSON.stringify(data, null, 2)}\n`;
}

function setByPointer(target: unknown, pointer: string, value: unknown) {
  if (!pointer.startsWith("/")) {
    throw new Error(`pointer must start with '/': ${pointer}`);
  }
  const parts = pointer.split("/").slice(1);
  // biome-ignore lint/suspicious/noExplicitAny: Ignore
  let obj: any = target;
  for (let i = 0; i < parts.length - 1; i++) {
    const key = parts[i];
    if (!(key in obj)) {
      obj[key] = {};
    }
    obj = obj[key];
  }
  obj[parts[parts.length - 1]] = value;
}

(async () => {
  for (const descriptor of files) {
    await updateFile(descriptor);
  }
})();
