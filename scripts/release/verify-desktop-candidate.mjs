// Boundary validation before a draft can reach cargo-dist's announcement.
import { readFileSync } from 'node:fs';
import { admitInstallerCompatibility, installerPairs, ReleaseTag,
  requireVerifiedInstallerAliases, isMainModule } from './cli-installer-compatibility.mjs';

const installers = [
  'Chatter-macos-apple-silicon.dmg', 'Chatter-macos-intel.dmg',
  'Chatter-windows-setup.exe', 'Chatter-linux-x86_64.AppImage',
  'Chatter-linux-x86_64.deb', 'latest.json',
];
const platforms = ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64', 'linux-x86_64'];

function admitDesktopDraft(release, manifest, identity, repo) {
  const tag = identity.value;
  if (release.tagName !== tag || release.isDraft !== true) {
    throw new Error('candidate must be an unpublished release for the exact version tag');
  }
  if (!Array.isArray(release.assets)) throw new Error('missing release assets');
  const assets = new Map();
  for (const asset of release.assets) {
    if (typeof asset.name !== 'string' || !Number.isSafeInteger(asset.size) || asset.size <= 0
        || assets.has(asset.name)) throw new Error('invalid, empty or duplicate asset');
    assets.set(asset.name, Object.freeze({ name: asset.name, size: asset.size }));
  }
  for (const name of installers) {
    if (!assets.has(name)) throw new Error(`missing installer or updater manifest: ${name}`);
  }
  if (manifest.version !== identity.version || !manifest.platforms
      || Object.keys(manifest.platforms).sort().join() !== [...platforms].sort().join()) {
    throw new Error('updater version or platform population differs');
  }
  const prefix = `https://github.com/${repo}/releases/download/${tag}/`;
  for (const key of platforms) {
    const entry = manifest.platforms[key];
    if (typeof entry.url !== 'string' || !entry.url.startsWith(prefix)
        || typeof entry.signature !== 'string' || !entry.signature.trim()) {
      throw new Error(`invalid updater identity or signature: ${key}`);
    }
    const name = entry.url.slice(prefix.length);
    if (!assets.has(name) || !assets.has(`${name}.sig`)) {
      throw new Error(`updater bundle or signature is not uploaded: ${key}`);
    }
  }
  return assets;
}

// A fully uploaded draft is the strict recovery/publication boundary.
export function verifyCandidate(release, manifest, tag, repo) {
  const assets = admitDesktopDraft(release, manifest, new ReleaseTag(tag), repo);
  for (const pair of installerPairs) {
    for (const name of [pair.current, pair.legacy]) {
      if (!assets.has(name)) throw new Error(`missing CLI installer: ${name}`);
    }
  }
}

// Current installers remain built announcement inputs, not fictional uploaded
// assets. Only byte-verified aliases may discharge this distinct prerequisite.
export function verifyPreAnnouncementCandidate(release, manifest, tag, repo, installerEvidence) {
  const identity = new ReleaseTag(tag);
  const assets = admitDesktopDraft(release, manifest, identity, repo);
  requireVerifiedInstallerAliases(installerEvidence, identity, assets);
}

const completeInventoryKey = Symbol('complete release inventory');
class CompleteReleaseInventory {
  #tag;
  constructor(key, identity) {
    if (key !== completeInventoryKey) throw new Error('inventory must come from complete candidate admission');
    this.#tag = identity;
    Object.freeze(this);
  }
  get tag() { return this.#tag.value; }
}
Object.freeze(CompleteReleaseInventory.prototype);

// A recovery must account for the producer's whole CLI/LSP population, not
// merely the installer aliases. This certifies inventory, not build provenance
// or the cryptographic checks performed by the artifact producer.
export function verifyCompleteCandidate(release, manifest, tag, repo, plan) {
  const identity = new ReleaseTag(tag);
  const compatibility = admitInstallerCompatibility(plan);
  compatibility.tag.assertSame(identity);
  const assets = admitDesktopDraft(release, manifest, identity, repo);
  for (const pair of installerPairs) {
    for (const name of [pair.current, pair.legacy]) {
      if (!assets.has(name)) throw new Error(`missing CLI installer: ${name}`);
    }
  }
  if (!plan.artifacts || typeof plan.artifacts !== 'object' || Array.isArray(plan.artifacts)
      || !Object.keys(plan.artifacts).length) throw new Error('missing producer artifact inventory');
  for (const [name, artifact] of Object.entries(plan.artifacts)) {
    if (typeof artifact?.name !== 'string' || artifact.name !== name || !name
        || name === '.' || name === '..' || /[\\/\x00-\x1f]/.test(name)) {
      throw new Error('invalid producer artifact identity');
    }
    if (!assets.has(name)) throw new Error(`missing planned release artifact: ${name}`);
  }
  for (const product of plan.releases) {
    if (product.app_version !== identity.version || !Array.isArray(product.artifacts) || !product.artifacts.length) {
      throw new Error('missing or cross-version product inventory');
    }
    for (const name of product.artifacts) {
      if (!Object.hasOwn(plan.artifacts, name)) throw new Error(`unresolved product artifact: ${name}`);
    }
  }
  for (const pair of installerPairs) {
    if (!Object.hasOwn(plan.artifacts, pair.current)) throw new Error('producer inventory omits a current installer');
  }
  return new CompleteReleaseInventory(completeInventoryKey, identity);
}

if (isMainModule(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args[0] === 'complete') {
    const [, release, manifest, tag, repo, plan] = args;
    if (args.length !== 6) throw new Error('expected complete draft metadata, updater, identity and producer plan');
    verifyCompleteCandidate(JSON.parse(readFileSync(release, 'utf8')), JSON.parse(readFileSync(manifest, 'utf8')),
      tag, repo, JSON.parse(readFileSync(plan, 'utf8')));
  } else if (args[0] === 'pre-announcement') {
    const [, release, manifest, tag, repo, installers, plan] = args;
    if (args.length !== 7) throw new Error('expected pre-announcement metadata, updater, identity and installer evidence');
    const evidence = admitInstallerCompatibility(JSON.parse(readFileSync(plan, 'utf8'))).verify(installers);
    verifyPreAnnouncementCandidate(JSON.parse(readFileSync(release, 'utf8')),
      JSON.parse(readFileSync(manifest, 'utf8')), tag, repo, evidence);
  } else {
    const [release, manifest, tag, repo] = args;
    if (!release || !manifest || !tag || !repo) throw new Error('expected release metadata, updater manifest, tag and repository');
    if (args.length !== 4) throw new Error('unexpected complete-draft arguments');
    verifyCandidate(JSON.parse(readFileSync(release, 'utf8')), JSON.parse(readFileSync(manifest, 'utf8')), tag, repo);
  }
}
