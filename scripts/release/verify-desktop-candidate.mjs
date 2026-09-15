// Boundary validation before a draft can reach cargo-dist's announcement.
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const installers = [
  'Chatter-macos-apple-silicon.dmg', 'Chatter-macos-intel.dmg',
  'Chatter-windows-setup.exe', 'Chatter-linux-x86_64.AppImage',
  'Chatter-linux-x86_64.deb', 'latest.json',
];
const platforms = ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64', 'linux-x86_64'];

export function verifyCandidate(release, manifest, tag, repo) {
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag)
      || release.tagName !== tag || release.isDraft !== true) {
    throw new Error('candidate must be an unpublished release for the exact version tag');
  }
  if (!Array.isArray(release.assets)) throw new Error('missing release assets');
  const names = new Set();
  for (const asset of release.assets) {
    if (typeof asset.name !== 'string' || !Number.isSafeInteger(asset.size) || asset.size <= 0
        || names.has(asset.name)) throw new Error('invalid, empty or duplicate asset');
    names.add(asset.name);
  }
  for (const name of installers) {
    if (!names.has(name)) throw new Error(`missing installer or updater manifest: ${name}`);
  }
  if (manifest.version !== tag.slice(1) || !manifest.platforms
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
    if (!names.has(name) || !names.has(`${name}.sig`)) {
      throw new Error(`updater bundle or signature is not uploaded: ${key}`);
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [release, manifest, tag, repo] = process.argv.slice(2);
  if (!release || !manifest || !tag || !repo) throw new Error('expected release metadata, updater manifest, tag and repository');
  verifyCandidate(JSON.parse(readFileSync(release, 'utf8')), JSON.parse(readFileSync(manifest, 'utf8')), tag, repo);
}
