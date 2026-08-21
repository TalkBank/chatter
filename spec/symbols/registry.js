'use strict';

// Shared reader and shared emit machinery for spec/symbols/symbol_registry.json.
//
// The registry has two kinds of entry and they are deliberately different
// shapes. `symbols` are ENTITIES: each has an identity (a codepoint), a name, a
// meaning, a parse role and a runnable example, and each maps 1:1 to a Rust
// enum variant. `character_classes` are SETS: bags of characters with no
// individual identity, used to build the grammar's word and event regexes.
// Storing a set as a list of records, or an entity as a bare character, would
// be the same category error in opposite directions.
//
// Everything downstream reads the registry through here, so "what does this
// symbol mean" has exactly one answer in the tree. The `ca_*` character arrays
// the grammar and the model consume are DERIVED here from `parse_role`; they
// are no longer written down anywhere.
//
// This module also owns the parts every generator needs and each used to carry
// its own copy of: `fail`, the Rust literal escaper, `rustfmt`, and the
// write-or-`--check` footer. Four copies of that footer existed once the
// generators went from two to four, and the two newest had already drifted from
// the two oldest in their STALE message. A generator directory that duplicates
// its own boilerplate is the defect this registry exists to remove, one level
// up.

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const PARSE_ROLES = ['word_attached', 'paired_stretch'];
const NOTATION_FAMILIES = ['conversation_analysis', 'disfluency'];

// The character-class keys, in the order the generated Rust constants list
// them. The `ca_*` keys are prepended when the categories view is built, so
// `Object.keys(categories)` IS the emission order and no generator restates it.
const CHARACTER_CLASS_KEYS = [
  'word_segment_forbidden_start_symbols',
  'word_segment_forbidden_rest_symbols',
  'word_segment_forbidden_common_symbols',
  'event_segment_forbidden_symbols',
  'event_segment_forbidden_common_symbols',
];

/** Builds the `fail` every script needs, differing only in its prefix. */
function failWith(prefix) {
  return function fail(message) {
    console.error(`${prefix}: ${message}`);
    process.exit(1);
  };
}

const fail = failWith('symbol registry');

/**
 * Looks up a closed-vocabulary key in a generator's payload table, or fails.
 *
 * Three generators each wrote this two-line guard for their own table, which is
 * the per-script duplication this module exists to end. The tables themselves
 * are correctly per-generator: a Rust type name and a book heading are prose for
 * one target and do not belong in a language-neutral registry.
 */
function lookupOrFail(table, key, tableName, fail) {
  const value = table[key];
  if (value === undefined) fail(`no entry for ${key}; add one to ${tableName}`);
  return value;
}

/** PascalCase of a snake_case id. Derives a Rust variant name, never stored. */
function rustVariant(id) {
  return id
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('');
}

/** The character a `U+XXXX` codepoint denotes. */
function charOf(codepoint) {
  const match = /^U\+([0-9A-F]{4,6})$/.exec(codepoint);
  if (!match) fail(`codepoint must look like U+2260, got ${JSON.stringify(codepoint)}`);
  return String.fromCodePoint(Number.parseInt(match[1], 16));
}

function isSingleUnicodeScalar(value) {
  return typeof value === 'string' && value.length > 0 && [...value].length === 1;
}

/**
 * Escapes any string as a Rust string literal.
 *
 * NOT `JSON.stringify`, which was briefly the second escaper in this directory.
 * JSON spells a control character `\uXXXX` and Rust spells it `\u{XX}`, and the
 * character classes really do contain U+0015, U+0001 through U+0004, U+0007,
 * U+0008 and the three ASCII whitespace controls, so the difference is
 * load-bearing rather than stylistic.
 */
function rustLiteral(text) {
  let out = '"';
  for (const ch of text) {
    const cp = ch.codePointAt(0);
    if (ch === '\\') out += '\\\\';
    else if (ch === '"') out += '\\"';
    else if (ch === '\n') out += '\\n';
    else if (ch === '\r') out += '\\r';
    else if (ch === '\t') out += '\\t';
    else if (cp < 0x20 || cp === 0x7f || cp === 0x2028 || cp === 0x2029) {
      out += `\\u{${cp.toString(16)}}`;
    } else out += ch;
  }
  return `${out}"`;
}

/** A Rust `char` pattern, by codepoint, for match arms. */
function rustCharPattern(ch) {
  return `'\\u{${ch.codePointAt(0).toString(16).toUpperCase()}}'`;
}

/**
 * Formats generated Rust with the same tool `cargo fmt` uses.
 *
 * Without this, two processes claim the same bytes: `just fmt` rewraps the
 * generated code and re-running the generator unwraps it again, so the two
 * disagree forever and each is correct. Running rustfmt here makes "generated"
 * and "formatted" one state. Found by adding `--check`, which reported a
 * committed file STALE on its first run against an unmodified registry.
 *
 * Hard failure when rustfmt is missing, deliberately: a generator that skipped
 * formatting would emit a file the next `cargo fmt` immediately invalidates.
 */
function rustfmt(source) {
  const result = spawnSync('rustfmt', ['--edition', '2024', '--emit', 'stdout'], {
    input: source,
    encoding: 'utf8',
  });
  if (result.error) {
    fail(`cannot run rustfmt (it ships with the Rust toolchain): ${result.error.message}`);
  }
  if (result.status !== 0) fail(`rustfmt rejected the generated module: ${result.stderr}`);
  return result.stdout;
}

/**
 * Whether this invocation was asked to compare rather than write.
 *
 * Read when asked, NOT captured at import. As a module-level const shared by
 * four scripts it was ambient state deciding whether a write happens: any
 * process that happened to carry an unrelated `--check` flipped every generator
 * into compare mode, and no single process could ever do both.
 */
function checkRequested() {
  return process.argv.includes('--check');
}

/**
 * Writes a generated artifact, or under `--check` compares and exits non-zero.
 *
 * `--check` exists so a test can gate these outputs: the generators write to
 * fixed repository paths, so a gate that simply ran them would mutate the tree
 * it is checking. Until 2026-08-11 nothing verified any of them and a hand-edit
 * to a generated file was undetectable.
 */
function writeGenerated(repoRoot, outputPath, content) {
  const relative = path.relative(repoRoot, outputPath);
  const existing = fs.existsSync(outputPath) ? fs.readFileSync(outputPath, 'utf8') : null;
  const matches = existing === content;
  if (checkRequested()) {
    if (!matches) {
      console.error(
        `STALE: ${outputPath} does not match spec/symbols/symbol_registry.json. ` +
          'Regenerate with `just symbols-gen`.',
      );
      // Sets the code rather than exiting, so a generator with more than one
      // output reports ALL of its stale files instead of only the first. The
      // process still exits non-zero, which is all the drift gate reads.
      process.exitCode = 1;
      return;
    }
    console.log(`current: ${relative}`);
    return;
  }
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  if (!matches) fs.writeFileSync(outputPath, content, 'utf8');
  // Says which of the two things happened. It used to say "updated" for a file
  // it had just decided not to write, so a no-op regeneration looked like five
  // rewrites.
  console.log(`${matches ? 'unchanged' : 'updated'}: ${relative}`);
}

/**
 * Reads and validates the registry.
 *
 * Returns exactly one shape for each fact, and only facts somebody reads. An
 * earlier cut also spread the raw JSON into the result, so callers could reach
 * an UNENRICHED `symbols` array with no `char` and no `rustVariant` sitting
 * beside the enriched one, and the character classes were reachable under two
 * names. In JavaScript both traps are silent. `description` is validated here
 * and deliberately not returned: no generator asks for it.
 */
function loadRegistry() {
  const repoRoot = path.resolve(__dirname, '..', '..');
  const registryPath = path.join(repoRoot, 'spec', 'symbols', 'symbol_registry.json');
  if (!fs.existsSync(registryPath)) fail(`missing registry file at ${registryPath}`);

  let raw;
  try {
    raw = JSON.parse(fs.readFileSync(registryPath, 'utf8'));
  } catch (err) {
    fail(`invalid JSON: ${err.message}`);
  }

  if (raw.version !== 3) fail(`expected registry version 3, got ${raw.version}`);
  if (!Array.isArray(raw.symbols)) fail('registry.symbols must be an array');
  if (typeof raw.description !== 'string' || raw.description.trim().length === 0) {
    fail('registry.description must be a non-empty string');
  }
  if (!raw.character_classes || typeof raw.character_classes !== 'object') {
    fail('registry.character_classes must be an object');
  }

  const seenIds = new Set();
  const seenCodepoints = new Set();
  const symbols = raw.symbols.map((entry, index) => {
    for (const field of ['id', 'codepoint', 'gloss', 'parse_role', 'notation_family', 'example']) {
      if (typeof entry[field] !== 'string' || entry[field].length === 0) {
        fail(`symbols[${index}] is missing a non-empty ${field}`);
      }
    }
    if (!PARSE_ROLES.includes(entry.parse_role)) {
      fail(`symbols[${index}] (${entry.id}) has unknown parse_role ${entry.parse_role}`);
    }
    if (!NOTATION_FAMILIES.includes(entry.notation_family)) {
      fail(`symbols[${index}] (${entry.id}) has unknown notation_family ${entry.notation_family}`);
    }
    if (!/^[a-z][a-z0-9_]*$/.test(entry.id)) {
      fail(`symbols[${index}] id must be snake_case, got ${entry.id}`);
    }
    if (seenIds.has(entry.id)) fail(`duplicate symbol id ${entry.id}`);
    if (seenCodepoints.has(entry.codepoint)) fail(`duplicate codepoint ${entry.codepoint}`);
    seenIds.add(entry.id);
    seenCodepoints.add(entry.codepoint);

    const char = charOf(entry.codepoint);
    if (!entry.example.includes(char)) {
      fail(`symbols[${index}] (${entry.id}) has an example that does not contain its own symbol`);
    }
    return { ...entry, char, rustVariant: rustVariant(entry.id) };
  });

  const byRole = (role) => symbols.filter((s) => s.parse_role === role).map((s) => s.char);

  // The view every generator consumes, in emission order. `ca_element_symbols`
  // and `ca_delimiter_symbols` are computed from parse_role and appear nowhere
  // in the registry file, so they cannot drift from the records.
  const categories = {
    ca_delimiter_symbols: byRole('paired_stretch'),
    ca_element_symbols: byRole('word_attached'),
  };
  for (const key of CHARACTER_CLASS_KEYS) {
    const values = raw.character_classes[key];
    if (!Array.isArray(values) || values.length === 0) {
      fail(`character class ${key} must be a non-empty array`);
    }
    const seen = new Set();
    for (const symbol of values) {
      if (!isSingleUnicodeScalar(symbol)) {
        fail(`${key} entries must be single Unicode scalar values, got ${JSON.stringify(symbol)}`);
      }
      if (seen.has(symbol)) fail(`${key} contains duplicate symbol ${JSON.stringify(symbol)}`);
      seen.add(symbol);
    }
    categories[key] = values;
  }

  return { repoRoot, symbols, categories };
}

module.exports = {
  PARSE_ROLES,
  NOTATION_FAMILIES,
  failWith,
  loadRegistry,
  lookupOrFail,
  rustCharPattern,
  rustLiteral,
  rustVariant,
  rustfmt,
  writeGenerated,
};
