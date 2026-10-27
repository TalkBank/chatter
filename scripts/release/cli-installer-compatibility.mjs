// The external installer-name compatibility contract, shared by publication
// and candidate validation. The executable remains `chatter`.
import { existsSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

export const installerPairs = Object.freeze(['sh', 'ps1'].map(extension => Object.freeze({
  extension,
  current: `talkbank-chatter-installer.${extension}`,
  legacy: `chatter-installer.${extension}`,
})));

// A symlinked CLI is still an invocation, not an import that may silently do
// nothing. Both release commands share this native filesystem identity check.
export function isMainModule(moduleUrl) {
  const entry = process.argv[1];
  return typeof entry === 'string' && existsSync(entry)
    && pathToFileURL(realpathSync(entry)).href === moduleUrl;
}

// SemVer 2.0 core/prerelease grammar (https://semver.org/), with the existing
// release policy of a leading v and no build metadata. The final assertion is
// absolute end-of-input: JavaScript's $ also accepts a trailing line terminator.
const releaseTagPattern = /^v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*)?(?![\s\S])/;

export class ReleaseTag {
  #value;
  constructor(value) {
    if (typeof value !== 'string' || !releaseTagPattern.test(value)) {
      throw new Error('expected an exact version tag');
    }
    this.#value = value;
    Object.freeze(this);
  }
  get value() { return this.#value; }
  get version() { return this.#value.slice(1); }
  assertVersion(version) {
    if (version !== this.version) throw new Error('release version differs from tag');
  }
  assertSame(other) {
    if (this.#value !== other.#value) throw new Error('installer evidence belongs to another release');
  }
}
Object.freeze(ReleaseTag.prototype);
Object.freeze(ReleaseTag);

const verifiedAliasesKey = Symbol('verified installer aliases');
const verifiedAliases = new WeakSet();
class VerifiedInstallerAliases {
  #tag;
  #members;
  constructor(key, tag, members) {
    if (key !== verifiedAliasesKey) throw new Error('installer evidence must come from byte verification');
    this.#tag = tag;
    this.#members = Object.freeze(members.map(member => Object.freeze({ ...member })));
    verifiedAliases.add(this);
    Object.freeze(this);
  }
  assertBeforeAnnouncement(tag, uploadedAssets) {
    this.#tag.assertSame(tag);
    for (const member of this.#members) {
      if (uploadedAssets.has(member.current)) throw new Error(`announcement input is already uploaded: ${member.current}`);
      if (uploadedAssets.get(member.legacy)?.size !== member.size) {
        throw new Error(`uploaded installer differs from verified bytes: ${member.legacy}`);
      }
    }
  }
}
Object.freeze(VerifiedInstallerAliases.prototype);

export function requireVerifiedInstallerAliases(evidence, tag, uploadedAssets) {
  if (!verifiedAliases.has(evidence)) throw new Error('missing verified installer evidence');
  evidence.assertBeforeAnnouncement(tag, uploadedAssets);
}

function currentInstallers(directory) {
  // Admit the complete pair before producing any legacy file.
  return installerPairs.map(pair => {
    const bytes = readFileSync(join(directory, pair.current));
    if (!bytes.length) throw new Error(`empty CLI installer: ${pair.current}`);
    return { ...pair, bytes };
  });
}

// Admit the producer's environment-variable mapping once; do not duplicate
// cargo-dist's list of settings. New explicit settings take precedence.
export function admitInstallerCompatibility(manifest) {
  const tag = new ReleaseTag(manifest?.announcement_tag);
  const releases = manifest?.releases?.filter(release => release.app_name === 'talkbank-chatter');
  if (releases?.length !== 1) throw new Error('expected exactly one talkbank-chatter release');
  tag.assertVersion(releases[0].app_version);
  const environment = releases[0].env;
  if (!environment?.install_dir_env_var || !environment?.no_modify_path_env_var) {
    throw new Error('missing installer environment mapping');
  }
  const variables = Object.values(environment).map(current => {
    if (typeof current !== 'string' || !/^TALKBANK_CHATTER_[A-Z_]+$/.test(current)) {
      throw new Error('invalid installer environment mapping');
    }
    return { current, legacy: current.replace(/^TALKBANK_CHATTER_/, 'CHATTER_') };
  });

  function wrapper(pair) {
    if (pair.extension === 'sh') {
      const bridge = variables.map(({ current, legacy }) =>
        `if [ -z "\${${current}:-}" ] && [ -n "\${${legacy}:-}" ]; then\n`
        + `    ${current}="$${legacy}"; export ${current}\nfi\n`).join('');
      return Buffer.concat([Buffer.from(`#!/bin/sh\n${bridge}`), pair.bytes]);
    }
    const bridge = variables.map(({ current, legacy }) =>
      `if ([string]::IsNullOrEmpty($env:${current}) -and -not [string]::IsNullOrEmpty($env:${legacy})) {\n`
      + `    $env:${current} = $env:${legacy}\n}\n`).join('');
    // param must remain the first statement; the generated installer is an
    // independent script block so its own param block retains that property.
    const script = 'param([switch]$NoModifyPath, [switch]$Help)\n' + bridge
      + `$installer = [scriptblock]::Create([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('${pair.bytes.toString('base64')}')))\n`
      + '& $installer @PSBoundParameters @args\n';
    return Buffer.from(script);
  }

  function verify(directory) {
    const members = [];
    for (const pair of currentInstallers(directory)) {
      const legacy = readFileSync(join(directory, pair.legacy));
      if (!legacy.equals(wrapper(pair))) throw new Error(`CLI installer wrapper differs: ${pair.legacy}`);
      members.push({ current: pair.current, legacy: pair.legacy, size: legacy.length });
    }
    return new VerifiedInstallerAliases(verifiedAliasesKey, tag, members);
  }
  return Object.freeze({
    tag,
    copyCurrent(source, destination) {
      const pairs = currentInstallers(source);
      for (const pair of pairs) {
        if (existsSync(join(destination, pair.current))) throw new Error(`retained installer exists: ${pair.current}`);
      }
      for (const pair of pairs) writeFileSync(join(destination, pair.current), pair.bytes, { flag: 'wx' });
    },
    create(directory) {
      const pairs = currentInstallers(directory);
      for (const pair of pairs) {
        if (existsSync(join(directory, pair.legacy))) throw new Error(`retained installer exists: ${pair.legacy}`);
      }
      for (const pair of pairs) {
        writeFileSync(join(directory, pair.legacy), wrapper(pair), { flag: 'wx' });
      }
      verify(directory);
    },
    verify,
  });
}

if (isMainModule(import.meta.url)) {
  const [mode, directory, destinationOrManifest, copyManifest] = process.argv.slice(2);
  const manifestPath = mode === 'copy-current' ? copyManifest : destinationOrManifest;
  if (!directory || !manifestPath) throw new Error('expected installer mode, directories and dist manifest');
  const compatibility = admitInstallerCompatibility(JSON.parse(readFileSync(manifestPath, 'utf8')));
  if (mode === 'copy-current') compatibility.copyCurrent(directory, destinationOrManifest);
  else if (mode === 'create') compatibility.create(directory);
  else if (mode === 'verify') compatibility.verify(directory);
  else throw new Error('expected copy-current|create|verify');
}
