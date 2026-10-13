import test from "node:test";
import assert from "node:assert/strict";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { admitDesktopSmokeBinary } from "../../scripts/desktop-smoke-binary.mjs";

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "desktop-smoke-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return {
    root,
    configUrl: pathToFileURL(join(root, "apps", "chatter-desktop", "wdio.conf.ts")).href,
    executable(path) {
      mkdirSync(dirname(path), { recursive: true });
      // An owned text file proves filesystem admission only, never app launch.
      writeFileSync(path, "discovery control\n");
      chmodSync(path, 0o700);
      return path;
    },
  };
}

for (const [platform, name] of [
  ["linux", "chatter-desktop"], ["win32", "chatter-desktop.exe"],
]) {
  test(`${platform} selects the workspace debug binary and native suffix`, (t) => {
    const f = fixture(t);
    const expected = f.executable(join(f.root, "target", "debug", name));
    // Leave a decoy at the old apps/target location: it must not be selected.
    f.executable(join(f.root, "apps", "target", "debug", name));
    assert.equal(admitDesktopSmokeBinary({ configUrl: f.configUrl, platform }).application, expected);
  });
}

test("an absolute target override selects its debug binary", (t) => {
  const f = fixture(t);
  const targetDir = join(f.root, "custom target");
  const expected = f.executable(join(targetDir, "debug", "chatter-desktop"));
  assert.equal(admitDesktopSmokeBinary({ configUrl: f.configUrl, platform: "linux", targetDir }).application, expected);
});

test("an exact binary override admits release or cross-target output without guessing", (t) => {
  const f = fixture(t);
  const binaryPath = f.executable(join(f.root, "custom target", "triple", "release", "chatter-desktop"));
  const admitted = admitDesktopSmokeBinary({ configUrl: f.configUrl, platform: "linux", binaryPath });
  assert.equal(admitted.application, binaryPath);
  assert.ok(Object.isFrozen(admitted));
});

test("unsupported platforms refuse even an existing explicit binary", (t) => {
  const f = fixture(t);
  const binaryPath = f.executable(join(f.root, "Chatter"));
  for (const platform of ["darwin", "freebsd"]) {
    assert.throws(() => admitDesktopSmokeBinary({ configUrl: f.configUrl, platform, binaryPath }), /does not support/);
  }
});

test("ambiguous, empty and relative overrides refuse before discovery", (t) => {
  const f = fixture(t);
  const options = { configUrl: f.configUrl, platform: "linux" };
  assert.throws(() => admitDesktopSmokeBinary({ ...options, binaryPath: f.root, targetDir: f.root }), /only one/);
  for (const key of ["binaryPath", "targetDir"]) {
    for (const value of ["", "relative/path"]) {
      assert.throws(() => admitDesktopSmokeBinary({ ...options, [key]: value }), /absolute path/);
    }
  }
});

test("missing or non-file binaries refuse without falling back to a decoy", (t) => {
  const f = fixture(t);
  const options = { configUrl: f.configUrl, platform: "linux" };
  f.executable(join(f.root, "apps", "target", "debug", "chatter-desktop"));
  assert.throws(() => admitDesktopSmokeBinary(options), /binary is not usable/);
  const binaryPath = f.executable(join(f.root, "target", "debug", "chatter-desktop"));
  assert.throws(() => admitDesktopSmokeBinary({ ...options, binaryPath: join(f.root, "missing") }), /binary is not usable/);
  assert.throws(() => admitDesktopSmokeBinary({ ...options, binaryPath: dirname(binaryPath) }), /binary is not usable/);
});

test("Linux refuses a regular file without execute permission", { skip: process.platform === "win32" }, (t) => {
  const f = fixture(t);
  const binaryPath = f.executable(join(f.root, "no-execute"));
  chmodSync(binaryPath, 0o600);
  assert.throws(() => admitDesktopSmokeBinary({ configUrl: f.configUrl, platform: "linux", binaryPath }), /binary is not usable/);
});
