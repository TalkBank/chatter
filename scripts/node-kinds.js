/**
 * The named node kinds of a tree-sitter `node-types.json`.
 *
 * A MODULE WITH NO SIDE EFFECTS, and that is the whole reason it exists as its
 * own file. `generate-node-types.js` is a SCRIPT: it reads the grammar and
 * writes the generated Rust to stdout at import time. Importing it to borrow
 * this one function therefore ran the generator, and
 * `check-node-type-docs.js` began printing 866 lines of Rust before its own
 * report. Sharing an owner is right; sharing it through a script is not.
 */

import path from 'path';

/**
 * The grammar directory to read, honouring an optional `argv[2]` override.
 *
 * SHARED for the same reason `namedKinds` is. The generator and the docs
 * checker run back to back under `just node-types-check`, and each resolved
 * this independently: passing a directory to one and not the other made the two
 * halves of one check answer about DIFFERENT grammars. De-duplicating the
 * filter and leaving the location behind fixed half the problem.
 *
 * @param {string} repoRoot absolute path to the repository root
 * @param {string[]} argv `process.argv`
 * @returns {string} absolute path to the grammar directory
 */
export function grammarDir(repoRoot, argv) {
  return argv[2] ? path.resolve(argv[2]) : path.resolve(repoRoot, 'grammar');
}

/**
 * Every named kind in a parsed `node-types.json`.
 *
 * Anonymous tokens (`,`, `.`) are excluded: they are already in the grammar and
 * carry no documentation.
 *
 * @param {Array<{type: string, named: boolean}>} nodeTypes parsed node-types.json
 * @returns {Set<string>} the named kinds
 */
export function namedKinds(nodeTypes) {
  const types = new Set();
  for (const node of nodeTypes) {
    if (node.named) {
      types.add(node.type);
    }
  }
  return types;
}
