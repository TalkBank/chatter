/* Compile with the same tree_sitter/parser.h as the generated parser.
 * Inspect compiled metadata, never a textual approximation of parser.c.
 * ABI 15 is deliberate: other layouts are refused before reading fields. */
#include "tree_sitter/parser.h"
#include <stddef.h>
#include <string.h>

#ifdef _WIN32
#define TSGU_METADATA_EXPORT __declspec(dllexport)
#else
#define TSGU_METADATA_EXPORT __attribute__((visibility("default")))
#endif
#ifndef TSGU_METADATA_SYMBOL
#define TSGU_METADATA_SYMBOL tsgu_nonmissing_kind_v15
#endif

TSGU_METADATA_EXPORT int TSGU_METADATA_SYMBOL(const void *raw, const unsigned char *kind,
                           size_t kind_length) {
  const TSLanguage *language = raw;
  if (!language || language->abi_version != 15) return -1;
  if (!kind || kind_length == 0) return 0;
  if (!language->symbol_names ||
      !language->symbol_metadata || !language->public_symbol_map ||
      language->token_count > language->symbol_count) return -2;

  /* Nonterminal alias maps need a separate proof. Refuse, rather than guess
   * their extent from an unbounded sentinel walk. */
  if (language->alias_map && language->alias_map[0] != 0) return 0;
  uint32_t candidate = language->symbol_count;
  for (uint32_t id = 0; id < language->symbol_count + language->alias_count; ++id) {
    const char *name = language->symbol_names[id];
    if (strlen(name) != kind_length || memcmp(name, kind, kind_length) != 0) continue;
    /* A duplicate name, lexical symbol, alias-only symbol, hidden/anonymous
     * kind or remapped symbol cannot supply this capability. */
    if (candidate != language->symbol_count || id < language->token_count ||
        id >= language->symbol_count || !language->symbol_metadata[id].named ||
        !language->symbol_metadata[id].visible ||
        language->public_symbol_map[id] != id) return 0;
    candidate = id;
  }
  if (candidate == language->symbol_count) return 0;
  for (uint32_t id = 0; id < language->symbol_count; ++id) {
    if (id != candidate && language->public_symbol_map[id] == candidate) return 0;
  }
  if (language->max_alias_sequence_length && !language->alias_sequences) return -2;
  for (uint32_t production = 0; production < language->production_id_count; ++production) {
    for (uint16_t child = 0; child < language->max_alias_sequence_length; ++child) {
      size_t index = (size_t)production * language->max_alias_sequence_length + child;
      /* Conservatively withhold capabilities for any aliased language until
       * alias-specific admission has its own engine-backed proof. */
      if (language->alias_sequences[index] != 0) return 0;
    }
  }
  return 1;
}
