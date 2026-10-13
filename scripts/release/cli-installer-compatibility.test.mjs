import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { admitInstallerCompatibility, installerPairs } from './cli-installer-compatibility.mjs';

const manifest = { releases: [{ app_name: 'talkbank-chatter', env: {
  install_dir_env_var: 'TALKBANK_CHATTER_INSTALL_DIR',
  no_modify_path_env_var: 'TALKBANK_CHATTER_NO_MODIFY_PATH',
  print_quiet_env_var: 'TALKBANK_CHATTER_PRINT_QUIET',
} }] };
const compatibility = admitInstallerCompatibility(manifest);
const createLegacyInstallers = directory => compatibility.create(directory);
const verifyInstallerAliases = directory => compatibility.verify(directory);

function installers(t) {
  const directory = mkdtempSync(join(tmpdir(), 'cli-installers-'));
  t.after(() => rmSync(directory, { recursive: true }));
  for (const pair of installerPairs) writeFileSync(join(directory, pair.current), `installer for ${pair.current}\n`);
  return directory;
}

test('legacy wrappers retain the complete producer payload', t => {
  const directory = installers(t);
  createLegacyInstallers(directory);
  verifyInstallerAliases(directory);
  for (const pair of installerPairs) {
    const current = readFileSync(join(directory, pair.current));
    const legacy = readFileSync(join(directory, pair.legacy));
    if (pair.extension === 'sh') assert.deepEqual(legacy.subarray(-current.length), current);
    else {
      const encoded = /FromBase64String\('([^']+)'\)/.exec(legacy.toString())[1];
      assert.deepEqual(Buffer.from(encoded, 'base64'), current);
      assert.match(legacy.toString(), /^param\(/);
      assert.match(legacy.toString(), /& \$installer @PSBoundParameters @args/);
    }
  }
});

test('shell boundary preserves legacy settings and arguments without overriding current settings', { skip: process.platform === 'win32' }, t => {
  const directory = installers(t);
  writeFileSync(join(directory, 'talkbank-chatter-installer.sh'),
    '#!/bin/sh\nprintf "%s\\n" "$TALKBANK_CHATTER_INSTALL_DIR" "$TALKBANK_CHATTER_NO_MODIFY_PATH" "$TALKBANK_CHATTER_PRINT_QUIET" "$@"\n');
  createLegacyInstallers(directory);
  const run = env => execFileSync('/bin/sh', [join(directory, 'chatter-installer.sh'), 'one arg', '--help'], { env, encoding: 'utf8' });
  const legacy = { CHATTER_INSTALL_DIR: '/custom install/bin', CHATTER_NO_MODIFY_PATH: '1', CHATTER_PRINT_QUIET: '1' };
  assert.equal(run(legacy), '/custom install/bin\n1\n1\none arg\n--help\n');
  assert.equal(run({ ...legacy, TALKBANK_CHATTER_INSTALL_DIR: '/new/bin', TALKBANK_CHATTER_NO_MODIFY_PATH: '0', TALKBANK_CHATTER_PRINT_QUIET: '0' }),
    '/new/bin\n0\n0\none arg\n--help\n');
});

test('missing, ambiguous or unsafe producer identities cannot construct compatibility', () => {
  const invalid = [
    {}, { releases: [] },
    { releases: [{ ...manifest.releases[0], app_name: 'other-package' }] },
    { releases: [manifest.releases[0], manifest.releases[0]] },
    { releases: [{ app_name: 'talkbank-chatter' }] },
    { releases: [{ app_name: 'talkbank-chatter', env: {} }] },
    { releases: [{ app_name: 'talkbank-chatter', env: { ...manifest.releases[0].env, install_dir_env_var: 'unsafe; command' } }] },
  ];
  for (const candidate of invalid) assert.throws(() => admitInstallerCompatibility(candidate));
});

test('missing or empty current evidence cannot produce a partial pair', t => {
  for (const pair of installerPairs) {
    for (const missing of [true, false]) {
      const directory = installers(t);
      if (missing) rmSync(join(directory, pair.current));
      else writeFileSync(join(directory, pair.current), '');
      assert.throws(() => createLegacyInstallers(directory));
      for (const alias of installerPairs) assert.equal(existsSync(join(directory, alias.legacy)), false);
    }
  }
});

test('every missing, empty or same-length-corrupt uploaded alias is refused', t => {
  for (const pair of installerPairs) {
    for (const corruption of ['missing', 'empty', 'different']) {
      const directory = installers(t);
      createLegacyInstallers(directory);
      const path = join(directory, pair.legacy);
      if (corruption === 'missing') rmSync(path);
      else if (corruption === 'empty') writeFileSync(path, '');
      else writeFileSync(path, Buffer.alloc(readFileSync(path).length, 120));
      assert.throws(() => verifyInstallerAliases(directory));
    }
  }
});

test('creation cannot overwrite an existing legacy artifact', t => {
  const directory = installers(t);
  const path = join(directory, installerPairs[0].legacy);
  writeFileSync(path, 'retained artifact');
  assert.throws(() => createLegacyInstallers(directory));
  assert.equal(readFileSync(path, 'utf8'), 'retained artifact');
});
