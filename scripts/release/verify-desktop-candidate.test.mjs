import { test } from 'node:test';
import assert from 'node:assert/strict';
import { verifyCandidate, verifyPreAnnouncementCandidate, verifyCompleteCandidate } from './verify-desktop-candidate.mjs';
import { admitInstallerCompatibility, installerPairs } from './cli-installer-compatibility.mjs';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

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

function completeCandidate() {
  const c = candidate();
  const names = [...installerPairs.map(pair => pair.current), 'cli.archive', 'cli.archive.sha256', 'lsp.archive'];
  const plan = { announcement_tag: c.tag,
    releases: [{ app_name: 'talkbank-chatter', app_version: c.manifest.version, artifacts: names.slice(0, 4),
      env: { install_dir_env_var: 'TALKBANK_CHATTER_INSTALL_DIR', no_modify_path_env_var: 'TALKBANK_CHATTER_NO_MODIFY_PATH' } },
    { app_name: 'talkbank-lsp', app_version: c.manifest.version, artifacts: ['lsp.archive'] }],
    artifacts: Object.fromEntries(names.map(name => [name, { name }])) };
  for (const name of names) if (!c.release.assets.some(asset => asset.name === name)) c.release.assets.push({ name, size: 100 });
  return { c, plan, verifyComplete: () => verifyCompleteCandidate(c.release, c.manifest, c.tag, c.repo, plan) };
}

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
