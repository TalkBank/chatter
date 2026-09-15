import { test } from 'node:test';
import assert from 'node:assert/strict';
import { verifyCandidate } from './verify-desktop-candidate.mjs';

function candidate() {
  const tag = 'v1.2.3', repo = 'example/project';
  const names = ['Chatter-macos-apple-silicon.dmg', 'Chatter-macos-intel.dmg',
    'Chatter-windows-setup.exe', 'Chatter-linux-x86_64.AppImage', 'Chatter-linux-x86_64.deb', 'latest.json'];
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
