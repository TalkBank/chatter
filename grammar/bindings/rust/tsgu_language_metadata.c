/* Compile with the same tree_sitter/parser.h as the generated parser.
 * Inspect compiled metadata, never a textual approximation of parser.c.
 * ABI 15 is deliberate: other layouts are refused before reading fields.
 *
 * Verdicts, in the order they are decided:
 *   -1  the compiled ABI is not 15;
 *   -2  the metadata layout is inconsistent;
 *   -3  the language has alias tables (refused for every kind, see below);
 *    0  this kind cannot be proven never MISSING;
 *    1  this kind is a unique, visible, named, unremapped nonterminal.
 * An empty kind asks the language-wide checks alone: 0 when they pass. */
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

/* Whether any production renames a child (an alias sequence entry) or any
 * nonterminal has contextual aliases (the alias map). Either can expose a
 * kind name on a node of another symbol, a MISSING token among them, and the
 * compiled tables do not say which symbol a production's child is, so no
 * kind of such a language is proven. This is a property of the LANGUAGE, not
 * of the kind asked about, so it is a verdict of its own rather than a 0 that
 * would blame whichever kind happened to be asked first. Proving kinds of an
 * aliased language needs its own engine-backed proof. */
static int tsgu_has_alias_tables(const TSLanguage *language) {
  if (language->alias_map && language->alias_map[0] != 0) return 1;
  for (uint32_t production = 0; production < language->production_id_count; ++production) {
    for (uint16_t child = 0; child < language->max_alias_sequence_length; ++child) {
      size_t index = (size_t)production * language->max_alias_sequence_length + child;
      if (language->alias_sequences[index] != 0) return 1;
    }
  }
  return 0;
}

TSGU_METADATA_EXPORT int TSGU_METADATA_SYMBOL(const void *raw, const unsigned char *kind,
                           size_t kind_length) {
  const TSLanguage *language = raw;
  if (!language || language->abi_version != 15) return -1;
  if (!language->symbol_names ||
      !language->symbol_metadata || !language->public_symbol_map ||
      language->token_count > language->symbol_count) return -2;
  if (language->max_alias_sequence_length && !language->alias_sequences) return -2;
  if (tsgu_has_alias_tables(language)) return -3;
  if (!kind || kind_length == 0) return 0;

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
  return 1;
}
