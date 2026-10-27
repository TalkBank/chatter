// Generate the Tauri updater `latest.json` manifest from per-platform sidecar
// files produced by the release-desktop workflow.
//
// The updater plugin (configured in tauri.conf.json) fetches this file from the
// GitHub Release to decide whether a newer version exists and where to get the
// signed update bundle. Its required shape (Tauri v2 static-JSON updater):
//
//   { "version", "pub_date", "notes",
//     "platforms": { "<os>-<arch>": { "url", "signature" }, ... } }
//
// where the platform key is one of darwin-aarch64 / darwin-x86_64 /
// windows-x86_64 / linux-x86_64, "url" points at the release asset, and
// "signature" is the literal content of the bundle's `.sig` file.
//
// The structure-building logic lives in `buildLatestJson` so it can be unit
// tested; `main` is the thin filesystem/CLI wrapper the workflow invokes.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const RELEASE_ASSET_BASE = "https://github.com";

// Producer and recovery verification share the exact supported population.
export { targetToPlatformKey } from "../../../scripts/release/updater-platforms.mjs";
import { CompleteUpdaterPlatforms } from "../../../scripts/release/updater-platforms.mjs";
import { ReleaseTag } from "../../../scripts/release/cli-installer-compatibility.mjs";

/// Build the `latest.json` object from platform entries and release metadata.
///
/// `entries` is an array of `{ key, file, signature }`; `meta` is
/// `{ version, tag, repo, pubDate, notes }`. Throws on an empty entry set,
/// missing fields, or duplicate platform keys: a malformed manifest would
/// strand every installed client, so fail loudly rather than emit junk.
export function buildLatestJson(entries, meta) {
  const admitted = CompleteUpdaterPlatforms.admit(entries);
  for (const field of ["version", "tag", "repo", "pubDate"]) {
    if (!meta?.[field]) {
      throw new Error(`latest.json: missing release metadata '${field}'`);
    }
  }
  const identity = new ReleaseTag(meta.tag);
  identity.assertVersion(meta.version);

  const platforms = {};
  for (const entry of admitted.entries) {
    platforms[entry.key] = {
      signature: entry.signature,
      url: `${RELEASE_ASSET_BASE}/${meta.repo}/releases/download/${identity.value}/${entry.file}`,
    };
  }

  return {
    version: identity.version,
    notes: meta.notes ?? "",
    pub_date: meta.pubDate,
    platforms,
  };
}

/// CLI: read every `*.json` sidecar in the directory given by the first
/// argument (each `{ key, file, signature }`), combine with release metadata
/// from the environment, and write `latest.json` to the second argument.
function main() {
  const [sidecarDir, outPath] = process.argv.slice(2);
  if (!sidecarDir || !outPath) {
    throw new Error("usage: generate-latest-json.mjs <sidecar-dir> <out-path>");
  }
  const entries = readdirSync(sidecarDir)
    .filter((name) => name.endsWith(".json"))
    .map((name) => JSON.parse(readFileSync(join(sidecarDir, name), "utf8")));

  const manifest = buildLatestJson(entries, {
    version: process.env.UPDATER_VERSION,
    tag: process.env.UPDATER_TAG,
    repo: process.env.UPDATER_REPO,
    pubDate: process.env.UPDATER_PUB_DATE,
    notes: process.env.UPDATER_NOTES ?? "",
  });
  writeFileSync(outPath, `${JSON.stringify(manifest, null, 2)}\n`);
}

// Only run main when invoked as a script, not when imported by tests.
// pathToFileURL handles Windows paths; the naive `file://${argv[1]}` only
// matches on POSIX (see write-sidecar.mjs for the failure it caused).
if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
