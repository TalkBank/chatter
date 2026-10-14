import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, readdirSync, rmSync,
  symlinkSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { installerPairs } from './cli-installer-compatibility.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));

// Reuse the gate's literal-run reader, not another YAML parser. Selection only
// locates the real boundary; the test executes its commands against an empty
// draft and actual built installer files.
function runBody(workflow, marker) {
  const reader = `
import importlib.util, json, sys, textwrap
from pathlib import Path
spec = importlib.util.spec_from_file_location('gate_sync', sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
print(json.dumps([textwrap.dedent(body) for _, body in module.workflow_run_steps(Path(sys.argv[2]))]))
`;
  const bodies = JSON.parse(execFileSync('python3', ['-c', reader,
    join(root, 'scripts/check_ci_gate_sync.py'), join(root, workflow)], { encoding: 'utf8' }));
  const matches = bodies.filter(body => body.includes(marker));
  assert.equal(matches.length, 1, `expected one executable boundary containing ${marker}`);
  return matches[0];
}

const fakeGithub = String.raw`
import { copyFileSync, existsSync, readdirSync, statSync, readFileSync, writeFileSync } from 'node:fs';
import { join, basename } from 'node:path';
const args = process.argv.slice(2);
const store = process.env.TEST_RELEASE_ASSETS;
const names = () => readdirSync(store).filter(name => name !== '.published');
const fail = message => { console.error(message); process.exit(1); };
if (args[0] !== 'release') fail('unexpected transport operation');
if (args[2] !== process.env.RELEASE_TAG) fail('cross-release transport operation');
if (args[1] === 'upload') {
  if (args.includes('--clobber')) fail('clobber is not an admitted transition');
  const files = [];
  for (let i = 3; i < args.length; i++) {
    if (args[i] === '--repo') {
      if (args[++i] !== process.env.GITHUB_REPOSITORY) fail('cross-repository upload');
    } else if (args[i].startsWith('--')) fail('unexpected upload option');
    else files.push(args[i]);
  }
  for (const file of files) {
    if (!existsSync(file)) fail('missing upload input: ' + file);
    if (existsSync(join(store, basename(file)))) fail('duplicate upload asset: ' + basename(file));
  }
  for (const file of files) {
    const destination = join(store, basename(file));
    copyFileSync(file, destination);
    if (basename(file) === process.env.TEST_CORRUPT_UPLOAD) {
      const bytes = readFileSync(destination);
      bytes[bytes.length - 1] ^= 1;
      writeFileSync(destination, bytes);
    }
  }
} else if (args[1] === 'download') {
  const pattern = args[args.indexOf('--pattern') + 1];
  const patterns = {
    'talkbank-chatter-installer.*': name => name.startsWith('talkbank-chatter-installer.'),
    'chatter-installer.*': name => name.startsWith('chatter-installer.'),
    '*chatter-installer.*': name => name.includes('chatter-installer.'),
  };
  if (!patterns[pattern]) fail('unexpected download selection');
  const matches = names().filter(patterns[pattern]);
  if (!matches.length) fail('no assets match the file pattern');
  const destination = args[args.indexOf('--dir') + 1];
  for (const name of matches) {
    if (existsSync(join(destination, name))) fail('download would overwrite evidence');
    copyFileSync(join(store, name), join(destination, name));
  }
} else if (args[1] === 'view') {
  console.log(JSON.stringify({ tagName: process.env.RELEASE_TAG, isDraft: !existsSync(join(store, '.published')),
    assets: names().map(name => ({ name, size: statSync(join(store, name)).size })) }));
} else if (args[1] === 'edit') {
  if (!args.includes('--draft=false')) fail('unexpected publication transition');
  for (const name of ['talkbank-chatter-installer.sh', 'talkbank-chatter-installer.ps1',
    'chatter-installer.sh', 'chatter-installer.ps1']) {
    if (!existsSync(join(store, name))) fail('publication before installer population is complete');
  }
  writeFileSync(join(store, '.published'), 'verified upload completed');
} else fail('unexpected transport operation');
`;

function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'release-publication-'));
  t.after(() => rmSync(directory, { recursive: true }));
  const work = join(directory, 'work'), runtime = join(directory, 'runtime');
  const store = join(directory, 'remote'), bin = join(directory, 'bin');
  for (const path of [work, runtime, store, bin, join(work, 'artifacts'), join(runtime, 'cli-dist-global')]) {
    mkdirSync(path, { recursive: true });
  }
  symlinkSync(join(root, 'scripts'), join(work, 'scripts'), 'dir');
  const executable = join(bin, 'github.mjs');
  writeFileSync(executable, fakeGithub);
  const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
  writeFileSync(join(bin, 'gh'), `#!/bin/sh\nexec ${quote(process.execPath)} ${quote(executable)} "$@"\n`, { mode: 0o755 });
  const plan = { announcement_tag: 'v1.2.3', releases: [{ app_name: 'talkbank-chatter', app_version: '1.2.3',
    env: { install_dir_env_var: 'TALKBANK_CHATTER_INSTALL_DIR', no_modify_path_env_var: 'TALKBANK_CHATTER_NO_MODIFY_PATH' } }] };
  for (const pair of installerPairs) {
    const bytes = `producer bytes for ${pair.current}\n`;
    writeFileSync(join(runtime, 'cli-dist-global', pair.current), bytes);
    writeFileSync(join(work, 'artifacts', pair.current), bytes);
  }
  const manifest = { version: '1.2.3', platforms: {} };
  const desktop = ['Chatter-macos-apple-silicon.dmg', 'Chatter-macos-intel.dmg', 'Chatter-windows-setup.exe',
    'Chatter-linux-x86_64.AppImage', 'Chatter-linux-x86_64.deb'];
  for (const platform of ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64', 'linux-x86_64']) {
    const name = `${platform}.bundle`;
    desktop.push(name, `${name}.sig`);
    manifest.platforms[platform] = { url: `https://github.com/example/project/releases/download/v1.2.3/${name}`,
      signature: 'signed-updater-evidence' };
  }
  for (const name of desktop) writeFileSync(join(store, name), 'built desktop evidence');
  writeFileSync(join(work, 'latest.json'), JSON.stringify(manifest));
  writeFileSync(join(store, 'latest.json'), JSON.stringify(manifest));
  const env = { ...process.env, PATH: `${bin}:${process.env.PATH}`, RUNNER_TEMP: runtime,
    TEST_RELEASE_ASSETS: store, RELEASE_TAG: plan.announcement_tag, GITHUB_REPOSITORY: 'example/project',
    DIST_PLAN: JSON.stringify(plan), RELEASE_COMMIT: '0123456789012345678901234567890123456789', PRERELEASE_FLAG: '' };
  function execute(body) {
    const expanded = body.replaceAll('${{ needs.plan.outputs.tag }}', plan.announcement_tag);
    assert.ok(!expanded.includes('${{'), 'unadmitted workflow expression in executed boundary');
    return spawnSync('/bin/bash', ['-e', '-u', '-o', 'pipefail', '-c', expanded], { cwd: work, env, encoding: 'utf8' });
  }
  const retain = () => execute(runBody('.github/workflows/release-desktop.yml', 'cli_installers='));
  const verify = () => execute(runBody('.github/workflows/release-desktop.yml', 'candidate-release.json'));
  const announce = () => execute(runBody('.github/workflows/release.yml', 'gh release upload'));
  return { directory, runtime, work, store, env, plan, retain, verify, announce };
}

function succeeded(result) {
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
}

test('actual publication steps consume built installers, verify uploaded aliases, then upload originals exactly once', t => {
  const f = fixture(t);
  succeeded(f.retain());
  for (const pair of installerPairs) {
    assert.ok(existsSync(join(f.store, pair.legacy)));
    assert.ok(!existsSync(join(f.store, pair.current)), 'announcement still owns the original upload');
  }
  succeeded(f.verify());
  assert.ok(!existsSync(join(f.store, '.published')));
  succeeded(f.announce());
  assert.ok(existsSync(join(f.store, '.published')));
});

test('every missing built installer refuses the real prerequisite before any alias is uploaded', t => {
  for (const pair of installerPairs) {
    const f = fixture(t);
    rmSync(join(f.runtime, 'cli-dist-global', pair.current));
    assert.notEqual(f.retain().status, 0);
    for (const alias of installerPairs) assert.ok(!existsSync(join(f.store, alias.legacy)));
    assert.ok(!existsSync(join(f.store, '.published')));
  }
});

test('same-length upload corruption is caught by the real downloaded-byte verification', t => {
  const f = fixture(t);
  f.env.TEST_CORRUPT_UPLOAD = installerPairs[0].legacy;
  const result = f.retain();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /CLI installer wrapper differs/);
  assert.ok(!existsSync(join(f.store, '.published')));
});

test('pre-uploading an original is not treated as safe announcement evidence', t => {
  const f = fixture(t);
  succeeded(f.retain());
  writeFileSync(join(f.store, installerPairs[0].current), 'unexpected existing asset');
  const result = f.verify();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /announcement input is already uploaded/);
  assert.ok(!existsSync(join(f.store, '.published')));
});

test('the consumer requests the global artifact emitted by the actual producer, not a release asset', () => {
  const producer = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8');
  const consumer = readFileSync(join(root, '.github/workflows/release-desktop.yml'), 'utf8');
  // These are transport wire identities; the executable test above proves what
  // the received files do. A missing/misrouted action must not be hidden by the
  // fake transport's prepared fixture directory.
  const emitted = /name: "?(artifacts-build-global)"?\s*\n/.exec(producer);
  assert.ok(emitted, 'producer must emit its global artifact');
  const downloaded = /- name: Download built CLI installer inputs from this run\n([\s\S]*?)(?=\n      - name:)/.exec(consumer);
  assert.ok(downloaded, 'consumer must acquire built evidence before preparation');
  assert.match(downloaded[1], /uses: actions\/download-artifact@/);
  assert.match(downloaded[1], new RegExp(`name: ${emitted[1]}\\s*\\n`));
  assert.match(downloaded[1], /path: \$\{\{ runner.temp \}\}\/cli-dist-global/);
});
