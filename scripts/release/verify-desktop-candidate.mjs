// Boundary validation before a draft can reach cargo-dist's announcement.
import { readFileSync } from 'node:fs';
import { CompleteUpdaterPlatforms } from './updater-platforms.mjs';
import { admitInstallerCompatibility, installerPairs, ReleaseTag,
  requireVerifiedInstallerAliases, isMainModule } from './cli-installer-compatibility.mjs';

const installers = [
  'Chatter-macos-apple-silicon.dmg', 'Chatter-macos-intel.dmg',
  'Chatter-windows-setup.exe', 'Chatter-linux-x86_64.AppImage',
  'Chatter-linux-x86_64.deb', 'latest.json',
];

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
  identity.assertVersion(manifest.version);
  if (!manifest.platforms
      || typeof manifest.platforms !== 'object' || Array.isArray(manifest.platforms)) {
    throw new Error('updater version or platform population differs');
  }
  const prefix = `https://github.com/${repo}/releases/download/${tag}/`;
  const entries = Object.entries(manifest.platforms).map(([key, entry]) => {
    if (typeof entry?.url !== 'string' || !entry.url.startsWith(prefix)) {
      throw new Error(`invalid updater identity: ${key}`);
    }
    return { key, file: entry.url.slice(prefix.length), signature: entry.signature };
  });
  const admitted = CompleteUpdaterPlatforms.admit(entries);
  for (const { key, file } of admitted.entries) {
    if (!assets.has(file) || !assets.has(`${file}.sig`)) {
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

// Shipping policy, independent of the untrusted producer inventory. Keep target
// changes coordinated with dist-workspace.toml and the platform acceptance plan.
const requiredCliTargets = new Set([
  'aarch64-apple-darwin', 'x86_64-apple-darwin',
  'x86_64-unknown-linux-musl', 'aarch64-unknown-linux-musl',
  'x86_64-pc-windows-msvc',
]);
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
  // The producer list is input, not proof of completeness. A plan that omits
  // the LSP and its artifacts must not mint CompleteReleaseInventory.
  const products = plan.releases;
  const requiredExecutables = new Map([['talkbank-chatter', 'chatter'], ['talkbank-lsp', 'talkbank-lsp']]);
  const requiredProducts = new Set(requiredExecutables.keys());
  if (!Array.isArray(products) || products.length !== requiredProducts.size) {
    throw new Error('release product population must contain exactly one CLI and one LSP');
  }
  for (const product of products) {
    if (!requiredProducts.delete(product?.app_name)) {
      throw new Error('release product population contains an unknown or duplicate product');
    }
  }
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
  for (const product of products) {
    if (product.app_version !== identity.version || !Array.isArray(product.artifacts) || !product.artifacts.length) {
      throw new Error('missing or cross-version product inventory');
    }
    for (const name of product.artifacts) {
      if (!Object.hasOwn(plan.artifacts, name)) throw new Error(`unresolved product artifact: ${name}`);
    }
    // Target coverage belongs to each product. An installer, checksum or a
    // different product's archive cannot discharge an executable obligation.
    const executable = requiredExecutables.get(product.app_name);
    const missingTargets = new Set(requiredCliTargets);
    for (const name of product.artifacts) {
      const artifact = plan.artifacts[name];
      if (artifact.kind !== 'executable-zip') continue;
      const targets = artifact.target_triples;
      if (!Array.isArray(targets) || targets.length === 0
          || new Set(targets).size !== targets.length) {
        throw new Error(`invalid product executable targets: ${name}`);
      }
      for (const target of targets) {
        if (!requiredCliTargets.has(target)) {
          throw new Error(`unsupported product executable target: ${target}`);
        }
        if (!Array.isArray(artifact.assets)
            || !artifact.assets.some(asset => asset?.kind === 'executable'
              && (asset.name === executable || asset.name === `${executable}.exe`))) {
          throw new Error(`missing product executable for ${product.app_name}: ${target}`);
        }
        missingTargets.delete(target);
      }
    }
    if (missingTargets.size) {
      throw new Error(`missing product executable targets for ${product.app_name}: ${[...missingTargets].join(', ')}`);
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
