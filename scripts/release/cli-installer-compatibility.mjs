// The external installer-name compatibility contract, shared by publication
// and candidate validation. The executable remains `chatter`.
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

export const installerPairs = ['sh', 'ps1'].map(extension => ({
  extension,
  current: `talkbank-chatter-installer.${extension}`,
  legacy: `chatter-installer.${extension}`,
}));

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
  const releases = manifest?.releases?.filter(release => release.app_name === 'talkbank-chatter');
  if (releases?.length !== 1) throw new Error('expected exactly one talkbank-chatter release');
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
    for (const pair of currentInstallers(directory)) {
      const legacy = readFileSync(join(directory, pair.legacy));
      if (!legacy.equals(wrapper(pair))) throw new Error(`CLI installer wrapper differs: ${pair.legacy}`);
    }
  }
  return Object.freeze({
    create(directory) {
      for (const pair of currentInstallers(directory)) {
        writeFileSync(join(directory, pair.legacy), wrapper(pair), { flag: 'wx' });
      }
      verify(directory);
    },
    verify,
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [mode, directory, manifestPath] = process.argv.slice(2);
  if (!directory || !manifestPath) throw new Error('expected create|verify, installer directory and dist manifest');
  const compatibility = admitInstallerCompatibility(JSON.parse(readFileSync(manifestPath, 'utf8')));
  if (mode === 'create') compatibility.create(directory);
  else if (mode === 'verify') compatibility.verify(directory);
  else throw new Error('expected create|verify, installer directory and dist manifest');
}
