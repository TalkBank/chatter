import test from "node:test";
import assert from "node:assert/strict";

import {
  buildLatestJson,
  targetToPlatformKey,
} from "../../scripts/generate-latest-json.mjs";

const META = {
  version: "0.1.1",
  tag: "v0.1.1",
  repo: "TalkBank/chatter",
  pubDate: "2026-06-16T00:00:00Z",
  notes: "Bug fixes.",
};

function sampleEntries() {
  return [
    { key: "darwin-aarch64", file: "Chatter_0.1.1_aarch64.app.tar.gz", signature: "SIG_MAC_ARM" },
    { key: "windows-x86_64", file: "Chatter_0.1.1_x64-setup.nsis.zip", signature: "SIG_WIN" },
  ];
}

test("buildLatestJson emits the Tauri-required top-level keys", () => {
  const manifest = buildLatestJson(sampleEntries(), META);
  assert.equal(manifest.version, "0.1.1");
  assert.equal(manifest.pub_date, META.pubDate);
  assert.equal(manifest.notes, "Bug fixes.");
  assert.ok(manifest.platforms, "platforms map must be present");
});

test("each platform entry carries the release-asset url and the raw signature", () => {
  const manifest = buildLatestJson(sampleEntries(), META);
  assert.deepEqual(manifest.platforms["darwin-aarch64"], {
    signature: "SIG_MAC_ARM",
    url: "https://github.com/TalkBank/chatter/releases/download/v0.1.1/Chatter_0.1.1_aarch64.app.tar.gz",
  });
  assert.equal(
    manifest.platforms["windows-x86_64"].url,
    "https://github.com/TalkBank/chatter/releases/download/v0.1.1/Chatter_0.1.1_x64-setup.nsis.zip",
  );
});

test("buildLatestJson rejects an empty platform set", () => {
  assert.throws(() => buildLatestJson([], META), /no platform entries/);
});

test("buildLatestJson rejects a duplicate platform key", () => {
  const dup = [...sampleEntries(), sampleEntries()[0]];
  assert.throws(() => buildLatestJson(dup, META), /duplicate platform key/);
});

test("buildLatestJson rejects an entry missing its signature", () => {
  const bad = [{ key: "linux-x86_64", file: "Chatter_0.1.1_amd64.AppImage" }];
  assert.throws(() => buildLatestJson(bad, META), /missing 'signature'/);
});

test("buildLatestJson rejects missing release metadata", () => {
  assert.throws(
    () => buildLatestJson(sampleEntries(), { ...META, tag: undefined }),
    /missing release metadata 'tag'/,
  );
});

test("targetToPlatformKey maps every shipped target triple to its Tauri key", () => {
  assert.equal(targetToPlatformKey("aarch64-apple-darwin"), "darwin-aarch64");
  assert.equal(targetToPlatformKey("x86_64-apple-darwin"), "darwin-x86_64");
  assert.equal(targetToPlatformKey("x86_64-pc-windows-msvc"), "windows-x86_64");
  assert.equal(targetToPlatformKey("x86_64-unknown-linux-gnu"), "linux-x86_64");
  assert.equal(targetToPlatformKey("aarch64-unknown-linux-gnu"), "linux-aarch64");
});

test("targetToPlatformKey throws on an unsupported triple instead of guessing", () => {
  assert.throws(
    () => targetToPlatformKey("riscv64gc-unknown-linux-gnu"),
    /unsupported target triple/,
  );
});
