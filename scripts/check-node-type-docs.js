#!/usr/bin/env node
/**
 * Fail if `node-type-docs.json` has drifted from the grammar.
 *
 * The docs file is the hand-written half of `node_types.rs`: the kind list
 * comes from `grammar/src/node-types.json`, the prose comes from here. Only the
 * generated output was ever checked, so this half drifted unnoticed and carried
 * six descriptions for node kinds the grammar had removed.
 *
 * Two directions, deliberately, and they are not the same severity:
 *
 *   STALE   an entry for a kind the grammar does not have. Always wrong, and
 *           it is how a description outlives the construct it describes.
 *   MISSING a kind with no entry. Not fatal, because the generator falls back
 *           to restating the name, but that fallback is not documentation and
 *           the count should not grow silently. Reported, not failed.
 */
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { grammarDir, namedKinds } from './node-kinds.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, '..');
const docs = JSON.parse(
  fs.readFileSync(path.join(here, 'node-type-docs.json'), 'utf8'),
);
// The SAME grammar directory the generator uses, resolved the same way. These
// two ran back to back under `just node-types-check` while disagreeing about
// which grammar to read whenever a directory was passed, so a caller could pass
// one grammar and have half the check silently answer about another.
const treeSitterDir = grammarDir(repoRoot, process.argv);
const nodeTypes = JSON.parse(
  fs.readFileSync(path.join(treeSitterDir, 'src', 'node-types.json'), 'utf8'),
);

// The generator OWNS this derivation; a second copy here is how the two halves
// would come to disagree about what counts as a named kind.
const kinds = namedKinds(nodeTypes);
const stale = Object.keys(docs).filter((k) => !kinds.has(k)).sort();
const missing = [...kinds].filter((k) => !(k in docs)).sort();

if (missing.length > 0) {
  console.error(
    `note: ${missing.length} node kind(s) have no description and will be ` +
      `generated with a placeholder that restates the name:\n  ${missing.join(', ')}`,
  );
}

if (stale.length > 0) {
  console.error(
    `error: node-type-docs.json describes ${stale.length} kind(s) the grammar ` +
      `does not have:\n  ${stale.join(', ')}\n` +
      'Remove them, or add the construct back to the grammar.',
  );
  process.exit(1);
}

console.log(
  `node-type-docs.json: ${Object.keys(docs).length} description(s), no stale entries`,
);
