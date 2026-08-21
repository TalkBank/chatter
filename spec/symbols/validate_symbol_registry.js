#!/usr/bin/env node

// Reports on spec/symbols/symbol_registry.json.
//
// This is a REPORT, not a gate. Every structural check now lives in
// registry.js, which every generator reads the registry through, so a malformed
// registry cannot reach a generator whether or not anyone runs this script.
// Leaving a second copy of those checks here would be the same duplication the
// registry exists to remove.
//
// One check was DELETED rather than moved: ca_delimiter_symbols and
// ca_element_symbols used to be two hand-written arrays that had to be proved
// disjoint. They are now derived from a single `parse_role` field, so a symbol
// in both is unrepresentable and there is nothing left to assert.

const { loadRegistry } = require('./registry.js');

function tally(items, key) {
  const counts = new Map();
  for (const item of items) counts.set(item[key], (counts.get(item[key]) ?? 0) + 1);
  return counts;
}

function main() {
  const { symbols, categories } = loadRegistry();

  console.log('symbol registry validation: ok');
  console.log(`  - symbols: ${symbols.length}`);
  for (const [role, count] of tally(symbols, 'parse_role')) {
    console.log(`      parse_role ${role}: ${count}`);
  }
  for (const [family, count] of tally(symbols, 'notation_family')) {
    console.log(`      notation_family ${family}: ${count}`);
  }
  for (const [key, values] of Object.entries(categories)) {
    console.log(`  - ${key}: ${values.length}${key.startsWith('ca_') ? ' (derived)' : ''}`);
  }
}

main();
