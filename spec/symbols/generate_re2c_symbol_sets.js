#!/usr/bin/env node

// Consume the same admitted character classes as the grammar and model.
const path = require('node:path');
const { loadRegistry, writeGenerated } = require('./registry.js');

// Unicode escapes avoid giving punctuation a second meaning in re2c syntax.
// loadRegistry has already admitted every member as a single Unicode scalar.
function scalars(...sets) {
  return [...new Set(sets.flat())]
    .sort((left, right) => left.codePointAt(0) - right.codePointAt(0))
    .map((character) => {
      const codepoint = character.codePointAt(0);
      return codepoint <= 0xffff
        ? `\\u${codepoint.toString(16).padStart(4, '0')}`
        : `\\U${codepoint.toString(16).padStart(8, '0')}`;
    }).join('');
}

function main() {
  const { repoRoot, categories: c } = loadRegistry();
  const markers = [...c.word_attached_symbols, ...c.paired_stretch_symbols];
  // NUL is the native lexer's physical end marker, not a CHAT character.
  const wordCommon = [...c.word_segment_forbidden_common_symbols, ...markers, '\0'];
  const eventCommon = [...c.event_segment_forbidden_common_symbols, ...markers, '\0'];
  const definitions = [
    `scope_label_char = [${scalars(c.scoped_marker_label_symbols)}];`,
    `ws_first = [^${scalars(c.word_segment_forbidden_start_symbols, wordCommon, ['0'])}];`,
    `ws_rest = [^${scalars(c.word_segment_forbidden_rest_symbols, wordCommon)}];`,
    `ev_char = [^${scalars(c.event_segment_forbidden_symbols, eventCommon)}];`,
    `w_ca_elem = [${scalars(c.word_attached_symbols)}];`,
    `w_ca_delim = [${scalars(c.paired_stretch_symbols)}];`,
  ];
  writeGenerated(repoRoot,
    path.join(repoRoot, 'crates/talkbank-parser-re2c/src/generated_symbol_sets.re'),
    '// Generated from spec/symbols/symbol_registry.json. Do not edit.\n'
      + '// Regenerate with just symbols-gen, then regenerate the native lexer.\n'
      + definitions.join('\n') + '\n');
}

main();
