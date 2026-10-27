// Shared producer/recovery boundary for the desktop platforms we ship.
const targets = new Map([
  ['aarch64-apple-darwin', 'darwin-aarch64'],
  ['x86_64-apple-darwin', 'darwin-x86_64'],
  ['x86_64-pc-windows-msvc', 'windows-x86_64'],
  ['x86_64-unknown-linux-gnu', 'linux-x86_64'],
]);
const requiredPlatforms = new Set(targets.values());
const admissionKey = Symbol('complete updater platforms');

export function targetToPlatformKey(target) {
  if (!targets.has(target)) throw new Error(`unsupported target triple for updater platform key: ${target}`);
  return targets.get(target);
}

// Only admission can produce a complete immutable set. This proves inventory
// and URL-safe asset identity, not architecture contents or signature validity.
export class CompleteUpdaterPlatforms {
  #entries;
  constructor(key, entries) {
    if (key !== admissionKey) throw new Error('updater platforms require complete admission');
    this.#entries = Object.freeze(entries.map(entry => Object.freeze({ ...entry })));
    Object.freeze(this);
  }
  static admit(entries) {
    if (!Array.isArray(entries) || entries.length === 0) throw new Error('latest.json: no platform entries provided');
    const seen = new Set(), files = new Set();
    for (const entry of entries) {
      for (const field of ['key', 'file', 'signature']) {
        if (typeof entry?.[field] !== 'string' || !entry[field].trim()) {
          throw new Error(`latest.json: entry missing '${field}'`);
        }
      }
      if (!requiredPlatforms.has(entry.key)) throw new Error(`unsupported updater platform: ${entry.key}`);
      if (seen.has(entry.key)) throw new Error(`latest.json: duplicate platform key '${entry.key}'`);
      if (entry.file === '.' || entry.file === '..' || /[\\/%?#\x00-\x20]/.test(entry.file)) {
        throw new Error('invalid updater asset basename');
      }
      if (files.has(entry.file)) throw new Error('shared updater bundle across distinct platforms');
      seen.add(entry.key);
      files.add(entry.file);
    }
    if (seen.size !== requiredPlatforms.size) throw new Error('incomplete updater platform population');
    return new CompleteUpdaterPlatforms(admissionKey, entries);
  }
  get entries() { return this.#entries; }
}
Object.freeze(CompleteUpdaterPlatforms.prototype);
Object.freeze(CompleteUpdaterPlatforms);
