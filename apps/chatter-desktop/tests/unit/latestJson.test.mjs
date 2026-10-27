import test from "node:test";
import { CompleteUpdaterPlatforms } from "../../../../scripts/release/updater-platforms.mjs";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

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
    { key: "darwin-x86_64", file: "Chatter-x86_64-apple-darwin.app.tar.gz", signature: "SIG_MAC_INTEL" },
    { key: "linux-x86_64", file: "Chatter_0.1.1_amd64.AppImage", signature: "SIG_LINUX" },
    { key: "windows-x86_64", file: "Chatter_0.1.1_x64-setup.exe", signature: "SIG_WIN" },
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
    "https://github.com/TalkBank/chatter/releases/download/v0.1.1/Chatter_0.1.1_x64-setup.exe",
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
  assert.throws(() => targetToPlatformKey("aarch64-unknown-linux-gnu"), /unsupported target triple/);
});

test("targetToPlatformKey throws on an unsupported triple instead of guessing", () => {
  assert.throws(
    () => targetToPlatformKey("riscv64gc-unknown-linux-gnu"),
    /unsupported target triple/,
  );
});

test("partial platform populations cannot produce an updater manifest", () => {
  assert.throws(() => buildLatestJson(sampleEntries().slice(0, 1), META), /platform population/);
});

test('every missing platform, foreign key, shared bundle and unsafe basename is refused', () => {
  for (let index = 0; index < sampleEntries().length; index++) {
    assert.throws(() => buildLatestJson(sampleEntries().filter((_, i) => i !== index), META), /platform population/);
  }
  for (const change of [
    entries => { entries[0].key = 'linux-aarch64'; },
    entries => { entries[1].file = entries[0].file; },
    ...['../bundle', 'bundle?query', 'bundle#fragment', 'bundle%2Fother', 'a\\b', 'bundle name'].map(file =>
      entries => { entries[0].file = file; }),
    entries => { entries[0].signature = ' \n'; },
  ]) {
    const entries = sampleEntries();
    change(entries);
    assert.throws(() => buildLatestJson(entries, META));
  }
  assert.deepEqual(buildLatestJson(sampleEntries().reverse(), META).platforms,
    buildLatestJson(sampleEntries(), META).platforms);
});

test('target mapping never guesses from architecture and operating-system substrings', () => {
  for (const target of ['x86_64-fake-linux-gnu', 'aarch64-apple-darwin-extra',
    'x86_64-pc-windows-gnu', 'aarch64-pc-windows-msvc', null, {}]) {
    assert.throws(() => targetToPlatformKey(target), /unsupported target triple/);
  }
});


test('generator CLI refuses partial sidecars without replacing an existing manifest', t => {
  const directory = mkdtempSync(join(tmpdir(), 'chatter-updater-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const output = join(directory, 'retained.txt');
  writeFileSync(output, 'retained manifest');
  const entries = sampleEntries();
  writeFileSync(join(directory, 'one.json'), JSON.stringify(entries[0]));
  const command = fileURLToPath(new URL('../../scripts/generate-latest-json.mjs', import.meta.url));
  const run = () => spawnSync(process.execPath, [command, directory, output], {
    encoding: 'utf8', env: { ...process.env, UPDATER_VERSION: META.version,
      UPDATER_TAG: META.tag, UPDATER_REPO: META.repo, UPDATER_PUB_DATE: META.pubDate },
  });
  const refused = run();
  assert.equal(refused.status, 1, refused.stderr);
  assert.match(refused.stderr, /platform population/);
  assert.equal(readFileSync(output, 'utf8'), 'retained manifest');
  for (const [index, entry] of entries.slice(1).entries()) {
    writeFileSync(join(directory, `other-${index}.json`), JSON.stringify(entry));
  }
  const accepted = run();
  assert.equal(accepted.status, 0, accepted.stderr);
  assert.deepEqual(JSON.parse(readFileSync(output, 'utf8')), buildLatestJson(entries, { ...META, notes: '' }));
});


test('complete platform evidence cannot be forged or changed through the input', () => {
  const entries = sampleEntries();
  const proof = CompleteUpdaterPlatforms.admit(entries);
  assert.throws(() => new CompleteUpdaterPlatforms({}, entries), /complete admission/);
  entries[0].file = 'different';
  entries.pop();
  assert.deepEqual(proof.entries, sampleEntries());
  assert.throws(() => { proof.entries[0].file = 'different'; }, TypeError);
});

test('updater producer refuses inconsistent or malformed release identity', () => {
  for (const meta of [
    { ...META, version: '0.1.2' },
    { ...META, tag: 'v01.1.1', version: '01.1.1' },
    { ...META, tag: 'v0.1.1-alpha..1', version: '0.1.1-alpha..1' },
    { ...META, tag: 'v0.1.1\n', version: '0.1.1\n' },
  ]) assert.throws(() => buildLatestJson(sampleEntries(), meta));
});

test('generator CLI refuses inconsistent release identity before replacing a manifest', t => {
  const directory = mkdtempSync(join(tmpdir(), 'chatter-updater-identity-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const output = join(directory, 'retained.txt');
  writeFileSync(output, 'retained manifest');
  for (const [index, entry] of sampleEntries().entries()) {
    writeFileSync(join(directory, `${index}.json`), JSON.stringify(entry));
  }
  const command = fileURLToPath(new URL('../../scripts/generate-latest-json.mjs', import.meta.url));
  const run = (version, tag) => spawnSync(process.execPath, [command, directory, output], {
    encoding: 'utf8', env: { ...process.env, UPDATER_VERSION: version, UPDATER_TAG: tag,
      UPDATER_REPO: META.repo, UPDATER_PUB_DATE: META.pubDate, UPDATER_NOTES: '' },
  });
  for (const [version, tag] of [['0.1.2', META.tag], ['0.1.1-alpha..1', 'v0.1.1-alpha..1']]) {
    const refused = run(version, tag);
    assert.equal(refused.status, 1, refused.stderr);
    assert.equal(readFileSync(output, 'utf8'), 'retained manifest');
  }
  const version = '0.1.1-rc.1';
  const accepted = run(version, `v${version}`);
  assert.equal(accepted.status, 0, accepted.stderr);
  assert.deepEqual(JSON.parse(readFileSync(output, 'utf8')), buildLatestJson(sampleEntries(), {
    ...META, version, tag: `v${version}`, notes: '',
  }));
});
