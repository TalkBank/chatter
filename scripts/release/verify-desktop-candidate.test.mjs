import { test } from 'node:test';
import assert from 'node:assert/strict';
import { verifyCandidate, verifyPreAnnouncementCandidate, verifyCompleteCandidate } from './verify-desktop-candidate.mjs';
import { admitInstallerCompatibility, installerPairs } from './cli-installer-compatibility.mjs';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

function candidate() {
  const tag = 'v1.2.3', repo = 'example/project';
  const names = ['Chatter-macos-apple-silicon.dmg', 'Chatter-macos-intel.dmg',
    'Chatter-windows-setup.exe', 'Chatter-linux-x86_64.AppImage', 'Chatter-linux-x86_64.deb', 'latest.json'];
  for (const pair of installerPairs) names.push(pair.current, pair.legacy);
  const platforms = {};
  for (const key of ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64', 'linux-x86_64']) {
    const name = `${key}.bundle`;
    names.push(name, `${name}.sig`);
    platforms[key] = { url: `https://github.com/${repo}/releases/download/${tag}/${name}`, signature: 'signed-bundle' };
  }
  return { release: { tagName: tag, isDraft: true, assets: names.map(name => ({name, size: 100})) },
    manifest: { version: '1.2.3', platforms }, tag, repo };
}
const verify = c => verifyCandidate(c.release, c.manifest, c.tag, c.repo);

test('complete draft may proceed to announcement', () => verify(candidate()));
test('every missing asset prevents announcement', () => {
  for (const asset of candidate().release.assets) {
    const c = candidate();
    c.release.assets = c.release.assets.filter(a => a.name !== asset.name);
    assert.throws(() => verify(c), undefined, asset.name);
  }
});
test('published, mismatched, empty and cross-release evidence is refused', () => {
  const changes = [
    c => { c.release.isDraft = false; },
    c => { c.release.tagName = 'v1.2.2'; },
    c => { c.manifest.version = '1.2.2'; },
    c => { c.release.assets[0].size = 0; },
    c => { delete c.manifest.platforms['darwin-aarch64']; },
    c => { c.manifest.platforms['darwin-aarch64'].signature = ''; },
    c => { c.manifest.platforms['darwin-aarch64'].url = 'https://example.org/bundle'; },
    c => { c.release.assets.push(c.release.assets[0]); },
  ];
  for (const change of changes) { const c = candidate(); change(c); assert.throws(() => verify(c)); }
});

function stagedCandidate(t, version = '1.2.3') {
  const c = candidate();
  const directory = mkdtempSync(join(tmpdir(), 'candidate-installers-'));
  t.after(() => rmSync(directory, { recursive: true }));
  const plan = { announcement_tag: `v${version}`, releases: [{ app_name: 'talkbank-chatter', app_version: version,
    env: { install_dir_env_var: 'TALKBANK_CHATTER_INSTALL_DIR', no_modify_path_env_var: 'TALKBANK_CHATTER_NO_MODIFY_PATH' } }] };
  for (const pair of installerPairs) writeFileSync(join(directory, pair.current), `producer input ${pair.current}\n`);
  const compatibility = admitInstallerCompatibility(plan);
  compatibility.create(directory);
  const evidence = compatibility.verify(directory);
  c.release.assets = c.release.assets.filter(asset => !installerPairs.some(pair => pair.current === asset.name));
  for (const pair of installerPairs) c.release.assets.find(asset => asset.name === pair.legacy).size = readFileSync(join(directory, pair.legacy)).length;
  const verifyStaged = proof => verifyPreAnnouncementCandidate(c.release, c.manifest, c.tag, c.repo, proof);
  return { c, evidence, verifyStaged };
}

test('pre-announcement admits verified incoming originals without pretending they are uploaded', t => {
  const { c, evidence, verifyStaged } = stagedCandidate(t);
  verifyStaged(evidence);
  assert.throws(() => verify(c), /missing CLI installer/);
  assert.ok(c.release.assets.every(asset => !installerPairs.some(pair => pair.current === asset.name)));
});

test('pre-announcement rejects raw, prototype-forged and constructor-forged evidence', t => {
  const { evidence, verifyStaged } = stagedCandidate(t);
  assert.throws(() => verifyStaged({}), /missing verified installer evidence/);
  assert.throws(() => verifyStaged(Object.create(Object.getPrototypeOf(evidence))), /missing verified installer evidence/);
  const forged = Object.create(Object.getPrototypeOf(evidence));
  Object.defineProperty(forged, 'assertBeforeAnnouncement', { value: () => {} });
  assert.throws(() => verifyStaged(forged), /missing verified installer evidence/);
  assert.throws(() => Object.defineProperty(Object.getPrototypeOf(evidence), 'assertBeforeAnnouncement', { value: () => {} }));
  assert.throws(() => new evidence.constructor({}, {}, []), /must come from byte verification/);
});

test('release identity and uploaded alias sizes must match the verified owner', t => {
  const other = stagedCandidate(t, '1.2.2');
  assert.throws(() => other.verifyStaged(other.evidence), /another release/);
  const same = stagedCandidate(t);
  same.c.release.assets.find(asset => asset.name === installerPairs[0].legacy).size += 1;
  assert.throws(() => same.verifyStaged(same.evidence), /differs from verified bytes/);
});

test('uploaded originals cannot masquerade as still-pending announcement inputs', t => {
  const { c, evidence, verifyStaged } = stagedCandidate(t);
  c.release.assets.push({ name: installerPairs[0].current, size: 1 });
  assert.throws(() => verifyStaged(evidence), /already uploaded/);
});

const cliTargets = [
  'aarch64-apple-darwin', 'x86_64-apple-darwin',
  'x86_64-unknown-linux-musl', 'aarch64-unknown-linux-musl',
  'x86_64-pc-windows-msvc',
];

function completeCandidate() {
  const c = candidate();
  const names = installerPairs.map(pair => pair.current);
  const plan = { announcement_tag: c.tag,
    releases: [{ app_name: 'talkbank-chatter', app_version: c.manifest.version, artifacts: [...names],
      env: { install_dir_env_var: 'TALKBANK_CHATTER_INSTALL_DIR', no_modify_path_env_var: 'TALKBANK_CHATTER_NO_MODIFY_PATH' } },
    { app_name: 'talkbank-lsp', app_version: c.manifest.version, artifacts: [] }],
    artifacts: Object.fromEntries(names.map(name => [name, { name }])) };
  for (const [index, stem, executable] of [[0, 'cli', 'chatter'], [1, 'lsp', 'talkbank-lsp']]) {
    for (const [targetIndex, target] of cliTargets.entries()) {
      const name = targetIndex === 0 ? `${stem}.archive` : `${stem}-${target}.archive`;
      const binary = target === 'x86_64-pc-windows-msvc' ? `${executable}.exe` : executable;
      plan.artifacts[name] = { name, kind: 'executable-zip', target_triples: [target],
        assets: [{ name: executable, kind: 'executable', path: binary }] };
      plan.artifacts[`${name}.sha256`] = { name: `${name}.sha256`, kind: 'checksum' };
      plan.releases[index].artifacts.push(name, `${name}.sha256`);
    }
  }
  for (const name of Object.keys(plan.artifacts)) {
    if (!c.release.assets.some(asset => asset.name === name)) c.release.assets.push({ name, size: 100 });
  }
  return { c, plan, verifyComplete: () => verifyCompleteCandidate(c.release, c.manifest, c.tag, c.repo, plan) };
}

function omitTarget(f, productIndex, target) {
  const product = f.plan.releases[productIndex];
  const names = product.artifacts.filter(name => f.plan.artifacts[name].target_triples?.includes(target));
  const omitted = new Set(names.flatMap(name => [name, `${name}.sha256`]));
  product.artifacts = product.artifacts.filter(name => !omitted.has(name));
  for (const name of omitted) delete f.plan.artifacts[name];
  f.c.release.assets = f.c.release.assets.filter(asset => !omitted.has(asset.name));
}

test('complete recovery refuses each omitted product target even when the plan and uploads agree', () => {
  for (const productIndex of [0, 1]) {
    for (const target of cliTargets) {
      const f = completeCandidate();
      omitTarget(f, productIndex, target);
      assert.throws(f.verifyComplete, /product executable/, `${productIndex}: ${target}`);
    }
  }
});

test('only executable archives with admitted target declarations satisfy platform coverage', () => {
  const changes = [
    f => { delete f.plan.artifacts['lsp.archive'].target_triples; },
    f => { f.plan.artifacts['lsp.archive'].target_triples = []; },
    f => { f.plan.artifacts['lsp.archive'].target_triples = 'aarch64-apple-darwin'; },
    f => { f.plan.artifacts['lsp.archive'].target_triples = [null]; },
    f => { f.plan.artifacts['lsp.archive'].target_triples = ['aarch64-apple-darwin', 'aarch64-apple-darwin']; },
    f => { f.plan.artifacts['lsp.archive'].target_triples = ['aarch64-apple-darwin', 'unknown-target']; },
    f => {
      omitTarget(f, 1, 'aarch64-apple-darwin');
      f.plan.artifacts['cli.archive.sha256'].target_triples = ['aarch64-apple-darwin'];
      f.plan.releases[1].artifacts.push('cli.archive.sha256');
    },
  ];
  for (const change of changes) {
    const f = completeCandidate(); change(f);
    assert.throws(f.verifyComplete, /product executable/);
  }
});

test('a multi-target executable archive can cover both macOS targets', () => {
  const f = completeCandidate();
  omitTarget(f, 1, 'x86_64-apple-darwin');
  f.plan.artifacts['lsp.archive'].target_triples.push('x86_64-apple-darwin');
  assert.equal(f.verifyComplete().tag, f.c.tag);
});

test('complete inventory admission includes every CLI/LSP artifact and checksum the producer declared', () => {
  const { c, plan, verifyComplete } = completeCandidate();
  assert.equal(verifyComplete().tag, c.tag);
  for (const name of Object.keys(plan.artifacts)) {
    const f = completeCandidate();
    f.c.release.assets = f.c.release.assets.filter(asset => asset.name !== name);
    assert.throws(f.verifyComplete, undefined, name);
  }
});

test('empty, cross-release, path-like or unresolved producer inventory cannot authorize recovery', () => {
  const changes = [
    f => { f.plan.artifacts = {}; },
    f => { f.plan.announcement_tag = 'v1.2.2'; },
    f => { f.plan.releases[1].app_version = '1.2.2'; },
    f => { f.plan.releases[1].artifacts = ['absent']; },
    f => { f.plan.artifacts['../outside'] = { name: '../outside' }; },
    f => { f.plan.artifacts['cli.archive'].name = 'different'; },
  ];
  for (const change of changes) { const f = completeCandidate(); change(f); assert.throws(f.verifyComplete); }
  const proof = completeCandidate().verifyComplete();
  assert.throws(() => new proof.constructor({}, {}), /complete candidate admission/);
});


test('complete inventory refuses an omitted LSP even when its artifacts are omitted too', () => {
  const f = completeCandidate();
  const omitted = new Set(f.plan.releases[1].artifacts);
  f.plan.releases = f.plan.releases.filter(product => product.app_name !== 'talkbank-lsp');
  for (const name of omitted) delete f.plan.artifacts[name];
  f.c.release.assets = f.c.release.assets.filter(asset => !omitted.has(asset.name));
  assert.throws(f.verifyComplete, /product population/);
});

test('product names alone cannot certify a missing or wrong executable', () => {
  const changes = [
    f => { f.plan.releases[1].artifacts = ['cli.archive']; delete f.plan.artifacts['lsp.archive'];
      f.c.release.assets = f.c.release.assets.filter(a => a.name !== 'lsp.archive'); },
    f => { f.plan.releases[1].artifacts = ['cli.archive.sha256']; },
    f => { delete f.plan.artifacts['lsp.archive'].assets; },
    f => { f.plan.artifacts['lsp.archive'].assets[0].kind = 'readme'; },
    f => { f.plan.artifacts['lsp.archive'].assets[0].name = 'chatter'; },
    f => { f.plan.artifacts['cli.archive'].kind = 'source-tarball'; },
  ];
  for (const change of changes) {
    const f = completeCandidate(); change(f);
    assert.throws(f.verifyComplete, /product executable/);
  }
  const windows = completeCandidate();
  windows.plan.artifacts['lsp-x86_64-pc-windows-msvc.archive'].assets[0].name = 'talkbank-lsp.exe';
  assert.equal(windows.verifyComplete().tag, windows.c.tag);
});


test('complete inventory refuses duplicate, unknown and unnamed products', () => {
  const changes = [
    f => { f.plan.releases[1].app_name = 'talkbank-chatter'; },
    f => { f.plan.releases[1].app_name = 'another-product'; },
    f => { delete f.plan.releases[1].app_name; },
    f => { f.plan.releases.push(structuredClone(f.plan.releases[1])); },
  ];
  for (const change of changes) {
    const f = completeCandidate();
    change(f);
    assert.throws(f.verifyComplete);
  }
  const reversed = completeCandidate();
  reversed.plan.releases.reverse();
  assert.equal(reversed.verifyComplete().tag, reversed.c.tag);
});

test('complete recovery command refuses a reduced product population', t => {
  const directory = mkdtempSync(join(tmpdir(), 'complete-release-'));
  t.after(() => rmSync(directory, { recursive: true }));
  const f = completeCandidate();
  const releasePath = join(directory, 'release.json');
  const updaterPath = join(directory, 'latest.json');
  const planPath = join(directory, 'plan.json');
  writeFileSync(updaterPath, JSON.stringify(f.c.manifest));
  const run = () => {
    writeFileSync(releasePath, JSON.stringify(f.c.release));
    writeFileSync(planPath, JSON.stringify(f.plan));
    return spawnSync(process.execPath, [fileURLToPath(new URL('./verify-desktop-candidate.mjs', import.meta.url)),
      'complete', releasePath, updaterPath, f.c.tag, f.c.repo, planPath], { encoding: 'utf8' });
  };
  const complete = run();
  assert.equal(complete.status, 0, complete.stderr);
  const preserved = structuredClone({ plan: f.plan, release: f.c.release });
  omitTarget(f, 1, 'aarch64-unknown-linux-musl');
  const reducedTargets = run();
  assert.equal(reducedTargets.status, 1, reducedTargets.stderr);
  assert.match(reducedTargets.stderr, /missing product executable targets/);
  assert.equal(reducedTargets.stdout, '');
  Object.assign(f.plan, preserved.plan);
  f.c.release = preserved.release;
  f.plan.releases[1].artifacts = ['cli.archive'];
  delete f.plan.artifacts['lsp.archive'];
  f.c.release.assets = f.c.release.assets.filter(asset => asset.name !== 'lsp.archive');
  const mislabeled = run();
  assert.equal(mislabeled.status, 1, mislabeled.stderr);
  assert.match(mislabeled.stderr, /product executable/);
  assert.equal(mislabeled.stdout, '');
  f.plan.releases = f.plan.releases.filter(product => product.app_name !== 'talkbank-lsp');
  const incomplete = run();
  assert.equal(incomplete.status, 1, incomplete.stderr);
  assert.match(incomplete.stderr, /product population/);
  assert.equal(incomplete.stdout, '');
});

test('distinct updater platforms cannot all reference one uploaded bundle', () => {
  const c = candidate();
  const first = c.manifest.platforms['darwin-aarch64'];
  for (const key of Object.keys(c.manifest.platforms)) c.manifest.platforms[key] = { ...first };
  assert.throws(() => verify(c), /shared updater bundle/);
});

test('mutually consistent metadata cannot admit a malformed release version', () => {
  for (const version of ['01.2.3', '1.2.3-alpha..1', '1.2.3-01', '1.2.3\n']) {
    const f = completeCandidate();
    const tag = `v${version}`;
    for (const entry of Object.values(f.c.manifest.platforms)) {
      entry.url = entry.url.replace(`/download/${f.c.tag}/`, `/download/${tag}/`);
    }
    f.c.tag = tag;
    f.c.release.tagName = tag;
    f.c.manifest.version = version;
    f.plan.announcement_tag = tag;
    for (const product of f.plan.releases) product.app_version = version;
    assert.throws(() => verify(f.c), /exact version tag/);
    assert.throws(f.verifyComplete, /exact version tag/);
  }
});
