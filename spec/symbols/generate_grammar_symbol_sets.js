#!/usr/bin/env node

// Emits the grammar's character-class constants from the symbol registry.
//
// Regex character classes need their own escaping, which is why this generator
// keeps `escapeRegexClassSymbol` rather than sharing the Rust literal escaper:
// a regex class escapes `[`, `]` and `^`, and a Rust literal does not.

const path = require('node:path');
const { loadRegistry, writeGenerated } = require('./registry.js');

// Every value reaching this has already been proved a single Unicode scalar by
// `loadRegistry`, which validates each character class as it builds the view. An
// earlier cut re-checked it here, which is a validity check reachable twice and
// was additionally a private copy of a predicate the shared module owns.
function escapeRegexClassSymbol(value) {
  const cp = value.codePointAt(0);

  switch (value) {
    case '\\':
      return '\\\\';
    case '[':
    case ']':
    case '^':
      return `\\${value}`;
    case '\t':
      return '\\t';
    case '\n':
      return '\\n';
    case '\r':
      return '\\r';
    default:
      if (cp < 0x20 || cp === 0x7f || cp === 0x2028 || cp === 0x2029) {
        return `\\u${cp.toString(16).padStart(4, '0')}`;
      }
      return value;
  }
}

function renderRawTemplateLiteral(text) {
  return text
    .replaceAll('`', '\\`')
    .replaceAll('${', '\\${');
}

function renderGeneratedFile(categories) {
  const pairedStretch = categories.paired_stretch_symbols.join('');
  const wordAttached = categories.word_attached_symbols.join('');
  // Every symbol in the registry, both roles. Forbidden inside a
  // `word_segment`, which is a statement about them all being structural
  // MARKERS, not about where their notation came from.
  const allMarkers = pairedStretch + wordAttached;

  const wordSegmentForbiddenStartBase = categories.word_segment_forbidden_start_symbols
    .map(escapeRegexClassSymbol)
    .join('');
  const wordSegmentForbiddenRestBase = categories.word_segment_forbidden_rest_symbols
    .map(escapeRegexClassSymbol)
    .join('');
  const wordSegmentForbiddenCommon = categories.word_segment_forbidden_common_symbols
    .map(escapeRegexClassSymbol)
    .join('');
  const eventSegmentForbiddenBase = categories.event_segment_forbidden_symbols
    .map(escapeRegexClassSymbol)
    .join('');
  const eventSegmentForbiddenCommon = categories.event_segment_forbidden_common_symbols
    .map(escapeRegexClassSymbol)
    .join('');

  return `/**
 * Generated file from spec/symbols/symbol_registry.json
 *
 * DO NOT EDIT MANUALLY.
 * To regenerate:
 *   just symbols-gen
 */

export const PAIRED_STRETCH_SYMBOLS = String.raw\`${renderRawTemplateLiteral(pairedStretch)}\`;
export const WORD_ATTACHED_SYMBOLS = String.raw\`${renderRawTemplateLiteral(wordAttached)}\`;
export const ALL_MARKER_SYMBOLS = String.raw\`${renderRawTemplateLiteral(allMarkers)}\`;

export const WORD_SEGMENT_FORBIDDEN_START_BASE = ${JSON.stringify(wordSegmentForbiddenStartBase)};
export const WORD_SEGMENT_FORBIDDEN_REST_BASE = ${JSON.stringify(wordSegmentForbiddenRestBase)};
export const WORD_SEGMENT_FORBIDDEN_COMMON = ${JSON.stringify(wordSegmentForbiddenCommon)};

export const EVENT_SEGMENT_FORBIDDEN_BASE = ${JSON.stringify(eventSegmentForbiddenBase)};
export const EVENT_SEGMENT_FORBIDDEN_COMMON = ${JSON.stringify(eventSegmentForbiddenCommon)};
`;
}

function main() {
  const { repoRoot, categories } = loadRegistry();
  writeGenerated(
    repoRoot,
    path.join(repoRoot, 'grammar', 'src', 'generated_symbol_sets.js'),
    renderGeneratedFile(categories),
  );
}

main();
