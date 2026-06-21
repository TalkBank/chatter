#!/usr/bin/env node

const fs = require('node:fs');
const path = require('node:path');

const REQUIRED_CATEGORY_KEYS = [
  'ca_delimiter_symbols',
  'ca_element_symbols',
  'word_segment_forbidden_start_symbols',
  'word_segment_forbidden_rest_symbols',
  'word_segment_forbidden_common_symbols',
  'event_segment_forbidden_symbols',
  'event_segment_forbidden_common_symbols',
];

function fail(message) {
  console.error(`grammar symbol-set generation failed: ${message}`);
  process.exit(1);
}

function readRegistry() {
  const repoRoot = path.resolve(__dirname, '..', '..');
  const registryPath = path.join(repoRoot, 'spec', 'symbols', 'symbol_registry.json');
  if (!fs.existsSync(registryPath)) {
    fail(`missing registry file at ${registryPath}`);
  }

  let registry;
  try {
    registry = JSON.parse(fs.readFileSync(registryPath, 'utf8'));
  } catch (err) {
    fail(`invalid JSON: ${err.message}`);
  }

  if (!registry.categories || typeof registry.categories !== 'object') {
    fail('registry.categories must be an object');
  }

  for (const key of REQUIRED_CATEGORY_KEYS) {
    if (!Array.isArray(registry.categories[key])) {
      fail(`missing or invalid category: ${key}`);
    }
  }

  return { repoRoot, registry };
}

function ensureSingleScalar(symbol) {
  if (typeof symbol !== 'string' || symbol.length === 0) {
    fail(`expected non-empty string, got: ${JSON.stringify(symbol)}`);
  }

  if ([...symbol].length !== 1) {
    fail(`expected single Unicode scalar value, got: ${JSON.stringify(symbol)}`);
  }

  return symbol;
}

function escapeRegexClassSymbol(symbol) {
  const value = ensureSingleScalar(symbol);
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

function renderGeneratedFile(registry) {
  const categories = registry.categories;
  const caDelimiter = categories.ca_delimiter_symbols.join('');
  const caElement = categories.ca_element_symbols.join('');
  const caAll = caDelimiter + caElement;

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

export const CA_DELIMITER_SYMBOLS = String.raw\`${renderRawTemplateLiteral(caDelimiter)}\`;
export const CA_ELEMENT_SYMBOLS = String.raw\`${renderRawTemplateLiteral(caElement)}\`;
export const CA_ALL_SYMBOLS = String.raw\`${renderRawTemplateLiteral(caAll)}\`;

export const WORD_SEGMENT_FORBIDDEN_START_BASE = ${JSON.stringify(wordSegmentForbiddenStartBase)};
export const WORD_SEGMENT_FORBIDDEN_REST_BASE = ${JSON.stringify(wordSegmentForbiddenRestBase)};
export const WORD_SEGMENT_FORBIDDEN_COMMON = ${JSON.stringify(wordSegmentForbiddenCommon)};

export const EVENT_SEGMENT_FORBIDDEN_BASE = ${JSON.stringify(eventSegmentForbiddenBase)};
export const EVENT_SEGMENT_FORBIDDEN_COMMON = ${JSON.stringify(eventSegmentForbiddenCommon)};
`;
}

function writeIfChanged(filePath, content) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const existing = fs.existsSync(filePath) ? fs.readFileSync(filePath, 'utf8') : null;
  if (existing !== content) {
    fs.writeFileSync(filePath, content, 'utf8');
  }
}

function main() {
  const { repoRoot, registry } = readRegistry();
  const content = renderGeneratedFile(registry);
  const outputPath = path.join(repoRoot, 'grammar', 'src', 'generated_symbol_sets.js');
  writeIfChanged(outputPath, content);
  console.log(`updated: ${path.relative(repoRoot, outputPath)}`);
}

main();
