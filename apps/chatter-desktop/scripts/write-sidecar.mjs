// Write a per-platform updater sidecar JSON (`{ key, file, signature }`) that
// the release-desktop workflow collects and `generate-latest-json.mjs`
// aggregates into latest.json. Each build job calls this once with its target
// triple, the updater bundle, and the bundle's `.sig`.
//
// The drift-prone target-triple -> platform-key mapping is reused from
// `generate-latest-json.mjs` (unit-tested in latestJson.test.mjs) instead of
// being hand-rolled in bash across the three OS jobs.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

import { targetToPlatformKey } from "./generate-latest-json.mjs";

function main() {
  const [target, bundlePath, sigPath, outDir] = process.argv.slice(2);
  if (!target || !bundlePath || !sigPath || !outDir) {
    throw new Error(
      "usage: write-sidecar.mjs <target-triple> <bundle> <sig> <out-dir>",
    );
  }
  const key = targetToPlatformKey(target);
  const sidecar = {
    key,
    file: basename(bundlePath),
    signature: readFileSync(sigPath, "utf8").trim(),
  };
  mkdirSync(outDir, { recursive: true });
  writeFileSync(join(outDir, `${key}.json`), `${JSON.stringify(sidecar)}\n`);
}

// pathToFileURL handles Windows paths (backslashes + drive letter); the naive
// `file://${process.argv[1]}` only matches on POSIX, so on Windows main() was
// skipped and no sidecar was written.
if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
