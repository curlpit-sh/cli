#!/usr/bin/env bun

import { $ } from "bun";

async function getDefaultBranch(): Promise<string> {
  try {
    const ref = (await $`git symbolic-ref --quiet refs/remotes/origin/HEAD`.text()).trim();
    const name = ref.split("/").pop();
    return name && name.length > 0 ? name : "main";
  } catch (error) {
    console.warn("warning: unable to determine origin default branch, assuming 'main'");
    return "main";
  }
}

async function getMergedBranches(base: string): Promise<Set<string>> {
  try {
    const output = await $`git branch --format='%(refname:short)' --merged origin/${base}`.text();
    return new Set(
      output
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line.length > 0),
    );
  } catch (error) {
    console.warn(
      "warning: failed to list merged branches, skipping merged cleanup",
      (error as Error).message ?? error,
    );
    return new Set();
  }
}

async function getGoneBranches(): Promise<Set<string>> {
  try {
    const output = await $`git branch -vv --no-color`.text();
    return new Set(
      output
        .split("\n")
        .map((line) => line.replace(/^\*?\s*/, ""))
        .filter((line) => line.includes(": gone]"))
        .map((line) => line.split(/\s+/)[0])
        .filter((line) => line.length > 0),
    );
  } catch (error) {
    console.warn(
      "warning: failed to detect branches with gone upstreams, skipping gone cleanup",
      (error as Error).message ?? error,
    );
    return new Set();
  }
}

async function pruneBranches(branches: Set<string>, opts: { force?: boolean } = {}) {
  for (const branch of branches) {
    const args = opts.force ? ["-D", branch] : ["-d", branch];
    try {
      await $`git branch ${args}`;
      console.log(`deleted ${opts.force ? "(force) " : ""}${branch}`);
    } catch (error) {
      console.warn(`warning: could not delete ${branch}:`, (error as Error).message ?? error);
    }
  }
}

async function main() {
  try {
    await $`git rev-parse --is-inside-work-tree`;
  } catch {
    console.error("error: this script must be run inside a git repository");
    process.exit(1);
  }

  console.log("Fetching remote state...");
  await $`git fetch --prune`;

  const defaultBranch = await getDefaultBranch();
  const exclusions = new Set(["HEAD", defaultBranch, "main", "master"]);

  const merged = await getMergedBranches(defaultBranch);
  const gone = await getGoneBranches();

  const mergedToDelete = new Set(
    [...merged].filter((branch) => !exclusions.has(branch)),
  );

  const goneToDelete = new Set(
    [...gone].filter((branch) => !exclusions.has(branch)),
  );

  if (mergedToDelete.size === 0 && goneToDelete.size === 0) {
    console.log("No merged or stale branches to delete.");
    return;
  }

  if (mergedToDelete.size > 0) {
    console.log("Deleting merged branches:");
    await pruneBranches(mergedToDelete);
  }

  if (goneToDelete.size > 0) {
    console.log("Deleting branches with gone upstreams:");
    await pruneBranches(goneToDelete, { force: true });
  }
}

await main();
