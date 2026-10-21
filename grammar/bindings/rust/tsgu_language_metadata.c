/* Compile with the same tree_sitter/parser.h as the generated parser.
 * Inspect compiled metadata, never a textual approximation of parser.c.
 * ABI 15 is deliberate: other layouts are refused before reading fields.
 *
 * Verdicts, in the order they are decided:
 *   -1  the compiled ABI is not 15;
 *   -2  the metadata layout is inconsistent;
 *   -3  an empty-kind query found alias tables;
 *    0  this kind cannot be proven never MISSING;
 *    1  this kind is a unique, visible, named, unremapped nonterminal
 *       that is not exposed by any contextual alias.
 * An empty kind asks the language-wide checks alone: 0 when they pass. */
#include "tree_sitter/parser.h"
#include <stddef.h>
#include <string.h>
#include <stdlib.h>

#ifdef _WIN32
#define TSGU_METADATA_EXPORT __declspec(dllexport)
#else
#define TSGU_METADATA_EXPORT __attribute__((visibility("default")))
#endif
#ifndef TSGU_METADATA_SYMBOL
#define TSGU_METADATA_SYMBOL tsgu_nonmissing_kind_v15
#endif

/* Validate every contextual alias before admitting a kind. The alias map
 * serves query matching, not TSNode kind construction; retain its presence
 * in the language-wide report without treating unrelated aliases as proof
 * failures. See the engine argument in the static-verification chapter. */
static int tsgu_alias_table_status(const TSLanguage *language) {
  int status = language->alias_map && language->alias_map[0] != 0 ? -3 : 0;
  for (uint32_t production = 0; production < language->production_id_count; ++production) {
    for (uint16_t child = 0; child < language->max_alias_sequence_length; ++child) {
      size_t index = (size_t)production * language->max_alias_sequence_length + child;
      uint16_t alias = language->alias_sequences[index];
      if (alias != 0) {
        if (alias >= language->symbol_count + language->alias_count) return -2;
        status = -3;
      }
    }
  }
  return status;
}

TSGU_METADATA_EXPORT int TSGU_METADATA_SYMBOL(const void *raw, const unsigned char *kind,
                           size_t kind_length) {
  const TSLanguage *language = raw;
  if (!language || language->abi_version != 15) return -1;
  /* TSSymbol is uint16_t; 65534 and 65535 are reserved error symbols.
   * Check before adding counts or walking any table. Subtraction is safe
   * because the first disjunct short-circuits oversized symbol counts. */
  if (language->symbol_count > UINT16_MAX - 1u ||
      language->alias_count > UINT16_MAX - 1u - language->symbol_count) return -2;
  if (!language->symbol_names ||
      !language->symbol_metadata || !language->public_symbol_map ||
      language->token_count > language->symbol_count) return -2;
  if (language->max_alias_sequence_length && !language->alias_sequences) return -2;
  int alias_status = tsgu_alias_table_status(language);
  if (alias_status == -2) return -2;
  if (!kind || kind_length == 0) return alias_status;

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
  /* A contextual alias could expose this kind on a missing lexical node.
   * No production-to-child-symbol reconstruction is needed: refuse every
   * such target, even if its actual aliased source is a nonterminal. */
  for (uint32_t production = 0; production < language->production_id_count; ++production) {
    for (uint16_t child = 0; child < language->max_alias_sequence_length; ++child) {
      size_t index = (size_t)production * language->max_alias_sequence_length + child;
      uint16_t alias = language->alias_sequences[index];
      if (alias != 0 && alias == candidate) return 0;
    }
  }
  return 1;
}

/* Required-position admission, for flat named sequences with at most one
 * optional kind. These are table proofs, not tree scans. The unsafe caller
 * supplies a genuine native generated language and its ABI-matching header;
 * pointer backing storage and library lifetime are not inferable from C
 * metadata. No Wasm language is supported by this bridge.
 *
 * Work/storage limits are refusal boundaries, never truncation. In addition
 * to these limits, each compressed row visits at most symbol_count symbols.
 * The graph is immutable after construction and shared by rule capabilities.
 */
#ifndef TSGU_REQUIRED_GRAPH_NEW
#define TSGU_REQUIRED_GRAPH_NEW tsgu_required_graph_new_v15
#endif
#ifndef TSGU_REQUIRED_GRAPH_DROP
#define TSGU_REQUIRED_GRAPH_DROP tsgu_required_graph_drop_v15
#endif
#ifndef TSGU_REQUIRED_RULE
#define TSGU_REQUIRED_RULE tsgu_required_rule_v15
#endif
#ifndef TSGU_REQUIRED_PREFIX
#define TSGU_REQUIRED_PREFIX tsgu_required_prefix_v15
#endif

enum {
  TSGU_MAX_STATES = 16384,
  TSGU_MAX_SYMBOLS = 4096,
  TSGU_MAX_CELLS = 16777216,
  TSGU_MAX_RECORDS = 1048576,
  TSGU_MAX_WORD = 32,
  TSGU_MAX_ACTION_VISITS = 16777216,
  TSGU_MAX_PATH_VISITS = 2 * TSGU_MAX_WORD * TSGU_MAX_RECORDS
};

typedef struct { TSStateId from, to; TSSymbol symbol; } TsguEdge;
typedef struct {
  TSStateId state;
  TSSymbol parent;
  uint16_t production;
  uint8_t width;
} TsguReduction;
typedef struct {
  const TSLanguage *language;
  TsguEdge *edges;
  TsguReduction *reductions;
  size_t edge_count, edge_capacity, reduction_count, reduction_capacity;
  TSSymbol root;
} TsguRequiredGraph;

TSGU_METADATA_EXPORT void TSGU_REQUIRED_GRAPH_DROP(void *raw) {
  TsguRequiredGraph *graph = raw;
  if (!graph) return;
  free(graph->edges);
  free(graph->reductions);
  free(graph);
}

static void *tsgu_grow(void *storage, size_t *capacity, size_t count, size_t size) {
  if (count < *capacity) return storage;
  if (*capacity >= TSGU_MAX_RECORDS) return NULL;
  /* Tiny grammars should not reserve hundreds of unused records. Geometric
   * growth still bounds the total copying for larger compiled tables. */
  const size_t next = *capacity ? *capacity * 2 : 16;
  if (next > TSGU_MAX_RECORDS || next > SIZE_MAX / size) return NULL;
  void *grown = realloc(storage, next * size);
  if (!grown) return NULL;
  *capacity = next;
  return grown;
}

/* Borrow a dense ABI-15 row directly; decode compressed groups once into the
 * caller's scratch row. This is the representation ts_language_lookup reads.
 * Duplicate or out-of-population symbols refuse the row. */
static const uint16_t *tsgu_table_row(const TSLanguage *language, uint32_t state,
                                     uint16_t *row) {
  const size_t symbols = language->symbol_count;
  if (state < language->large_state_count) {
    return language->parse_table + state * symbols;
  }
  memset(row, 0, symbols * sizeof(*row));
  bool seen[TSGU_MAX_SYMBOLS] = {false};
  const uint16_t *cursor = language->small_parse_table +
    language->small_parse_table_map[state - language->large_state_count];
  const uint16_t groups = *cursor++;
  if (groups > symbols) return NULL;
  size_t visited = 0;
  for (uint16_t group = 0; group < groups; ++group) {
    const uint16_t value = *cursor++;
    const uint16_t count = *cursor++;
    if (count > symbols - visited) return NULL;
    visited += count;
    for (uint16_t entry = 0; entry < count; ++entry) {
      const uint16_t symbol = *cursor++;
      if (symbol >= symbols || seen[symbol]) return NULL;
      seen[symbol] = true;
      row[symbol] = value;
    }
  }
  return row;
}

static bool tsgu_add_edge(TsguRequiredGraph *graph, uint32_t from,
                          uint32_t to, uint32_t symbol) {
  if (to >= graph->language->state_count || to == 1) return false;
  if (to == 0) return true;
  TsguEdge *grown = tsgu_grow(graph->edges, &graph->edge_capacity,
                            graph->edge_count, sizeof(TsguEdge));
  if (!grown) return false;
  graph->edges = grown;
  graph->edges[graph->edge_count++] = (TsguEdge){from, to, symbol};
  return true;
}

static bool tsgu_required_symbol(const TSLanguage *, const char *, TSSymbol *);

static bool tsgu_populate_graph(TsguRequiredGraph *graph, bool *accepts) {
  const TSLanguage *language = graph->language;
  uint16_t scratch[TSGU_MAX_SYMBOLS];
  size_t visits = 0;
  for (uint32_t state = 0; state < language->state_count; ++state) {
    const uint16_t *row = tsgu_table_row(language, state, scratch);
    if (!row) return false;
    for (uint32_t symbol = 0; symbol < language->symbol_count; ++symbol) {
      if (symbol >= language->token_count) {
        if (state == 0) { if (row[symbol]) return false; }
        else if (!tsgu_add_edge(graph, state, row[symbol], symbol)) return false;
        continue;
      }
      const TSParseActionEntry *entry = language->parse_actions + row[symbol];
      const size_t count = entry->entry.count;
      if (count > TSGU_MAX_ACTION_VISITS - visits) return false;
      visits += count;
      for (size_t i = 1; i <= count; ++i) {
        const TSParseAction action = entry[i].action;
        if (state == 0) {
          if (action.type != TSParseActionTypeRecover) return false;
          continue;
        }
        switch (action.type) {
          case TSParseActionTypeShift:
            if (action.shift.extra || !tsgu_add_edge(graph, state, action.shift.state, symbol))
              return false;
            break;
          case TSParseActionTypeReduce: {
            if (action.reduce.symbol < language->token_count ||
                action.reduce.symbol >= language->symbol_count ||
                action.reduce.production_id >= language->production_id_count) return false;
            TsguReduction *grown = tsgu_grow(graph->reductions, &graph->reduction_capacity,
                                            graph->reduction_count, sizeof(TsguReduction));
            if (!grown) return false;
            graph->reductions = grown;
            graph->reductions[graph->reduction_count++] = (TsguReduction){
              state, action.reduce.symbol, action.reduce.production_id, action.reduce.child_count
            };
            break;
          }
          case TSParseActionTypeAccept:
            if (symbol != 0) return false;
            accepts[state] = true;
            break;
          case TSParseActionTypeRecover:
            break;
          default:
            return false;
        }
      }
    }
  }
  return graph->edge_count != 0 && graph->reduction_count != 0;
}

/* Null means refusal (including allocation/work limits), never an empty
 * admitted graph. Only this function allocates a graph passed to the other
 * two entrypoints. The native language must outlive it. */
TSGU_METADATA_EXPORT void *TSGU_REQUIRED_GRAPH_NEW(const void *raw) {
  const TSLanguage *language = raw;
  if (!language || language->abi_version != 15) return NULL;
  if (language->state_count < 2 || language->state_count > TSGU_MAX_STATES ||
      !language->token_count || language->token_count >= language->symbol_count ||
      language->symbol_count > TSGU_MAX_SYMBOLS ||
      language->alias_count > TSGU_MAX_SYMBOLS - language->symbol_count ||
      language->state_count > TSGU_MAX_CELLS / language->symbol_count ||
      language->production_id_count > TSGU_MAX_RECORDS ||
      (language->max_alias_sequence_length &&
       language->production_id_count > TSGU_MAX_CELLS / language->max_alias_sequence_length) ||
      language->large_state_count > language->state_count ||
      (language->large_state_count && !language->parse_table) ||
      (language->large_state_count < language->state_count &&
       (!language->small_parse_table || !language->small_parse_table_map)) ||
      !language->parse_actions || !language->symbol_names || !language->symbol_metadata ||
      !language->public_symbol_map ||
      (language->max_alias_sequence_length && !language->alias_sequences)) return NULL;
  if (tsgu_alias_table_status(language) == -2) return NULL;
  for (uint32_t id = 0; id < language->symbol_count + language->alias_count; ++id) {
    if (!language->symbol_names[id]) return NULL;
    size_t length = 0;
    while (length < 4096 && language->symbol_names[id][length]) ++length;
    if (length == 4096) return NULL;
  }
  for (uint32_t id = 0; id < language->symbol_count; ++id) {
    if (language->public_symbol_map[id] >= language->symbol_count + language->alias_count)
      return NULL;
  }
  TsguRequiredGraph *graph = calloc(1, sizeof(*graph));
  bool *accepts = calloc(language->state_count, sizeof(*accepts));
  bool *incoming = calloc(language->state_count, sizeof(*incoming));
  if (!graph || !accepts || !incoming) {
    free(graph); free(accepts); free(incoming); return NULL;
  }
  graph->language = language;
  bool valid = tsgu_populate_graph(graph, accepts);
  size_t root_edges = 0, accept_states = 0;
  for (size_t i = 0; valid && i < graph->edge_count; ++i) {
    const TsguEdge edge = graph->edges[i];
    if (!accepts[edge.to]) continue;
    if (edge.from != 1 || edge.symbol < language->token_count ||
        (root_edges && edge.symbol != graph->root)) { valid = false; break; }
    graph->root = edge.symbol;
    incoming[edge.to] = true;
    ++root_edges;
  }
  for (uint32_t state = 1; valid && state < language->state_count; ++state) {
    if (accepts[state]) { ++accept_states; valid = incoming[state]; }
  }
  free(accepts); free(incoming);
  TSSymbol canonical_root = 0;
  if (!valid || !root_edges || !accept_states ||
      !tsgu_required_symbol(language, language->symbol_names[graph->root], &canonical_root) ||
      canonical_root != graph->root) {
    TSGU_REQUIRED_GRAPH_DROP(graph); return NULL;
  }
  return graph;
}

/* Unique canonical named identity, with no incoming remap. Contextual alias
 * exposure is excluded globally for parents as well as child symbols. */
static bool tsgu_required_symbol(const TSLanguage *language, const char *name, TSSymbol *out) {
  if (!name || !*name) return false;
  uint32_t candidate = language->symbol_count;
  for (uint32_t id = 0; id < language->symbol_count + language->alias_count; ++id) {
    if (strcmp(language->symbol_names[id], name)) continue;
    if (candidate != language->symbol_count || id >= language->symbol_count ||
        !language->symbol_metadata[id].named || !language->symbol_metadata[id].visible ||
        language->public_symbol_map[id] != id) return false;
    candidate = id;
  }
  if (candidate == language->symbol_count) return false;
  for (uint32_t id = 0; id < language->symbol_count; ++id) {
    if (id != candidate && language->public_symbol_map[id] == candidate) return false;
  }
  for (size_t i = 0; i < (size_t)language->production_id_count * language->max_alias_sequence_length; ++i) {
    if (language->alias_sequences[i] == candidate) return false;
  }
  *out = (TSSymbol)candidate;
  return true;
}

/* Does EVERY complete length-width graph path into EVERY selected reduction
 * start with this word? All nonzero states are starting states. Reachability and
 * mismatch propagate independently; merging a good path cannot hide a bad
 * one. Missing paths/refused arities never establish the property vacuously. */
static bool tsgu_required_path_prefix(const TsguRequiredGraph *graph, TSSymbol parent,
                                      const TSSymbol *word, size_t count, size_t width) {
  if (!count || count > width) return false;
  const size_t states = graph->language->state_count;
  bool *memory = calloc(4 * states, sizeof(bool));
  if (!memory) return false;
  bool *reach = memory, *bad = memory + states;
  bool *next = memory + 2 * states, *next_bad = memory + 3 * states;
  for (size_t state = 1; state < states; ++state) reach[state] = true;
  for (size_t depth = 0; depth < width; ++depth) {
    memset(next, 0, states * sizeof(bool));
    memset(next_bad, 0, states * sizeof(bool));
    for (size_t i = 0; i < graph->edge_count; ++i) {
      const TsguEdge edge = graph->edges[i];
      if (!reach[edge.from]) continue;
      next[edge.to] = true;
      if (bad[edge.from] || (depth < count && edge.symbol != word[depth]))
        next_bad[edge.to] = true;
    }
    bool *swap = reach; reach = next; next = swap;
    swap = bad; bad = next_bad; next_bad = swap;
  }
  size_t population = 0;
  bool valid = true;
  for (size_t i = 0; i < graph->reduction_count; ++i) {
    const TsguReduction reduction = graph->reductions[i];
    if (reduction.parent != parent || reduction.width != width) continue;
    ++population;
    if (!reach[reduction.state] || bad[reduction.state]) { valid = false; break; }
  }
  free(memory);
  return valid && population != 0;
}

static bool tsgu_required_word(const TsguRequiredGraph *graph, TSSymbol parent,
                               const TSSymbol *word, size_t width) {
  return tsgu_required_path_prefix(graph, parent, word, width, width);
}

/* children is the longer exact word. optional == count means no optional
 * member; otherwise the second exact word omits that one member. Strings
 * and the array are borrowed only for this call. Returns 1 only on proof. */
TSGU_METADATA_EXPORT int TSGU_REQUIRED_RULE(const void *raw, const char *parent_name,
                                            const char *const *children, size_t count,
                                            size_t optional) {
  const TsguRequiredGraph *graph = raw;
  if (!graph || !children || !count || count > TSGU_MAX_WORD || optional > count ||
      (optional < count && count < 2)) return 0;
  const TSLanguage *language = graph->language;
  TSSymbol parent, word[TSGU_MAX_WORD], short_word[TSGU_MAX_WORD];
  if (!tsgu_required_symbol(language, parent_name, &parent) ||
      parent < language->token_count || parent == graph->root) return 0;
  size_t short_count = 0;
  for (size_t i = 0; i < count; ++i) {
    if (!tsgu_required_symbol(language, children[i], &word[i])) return 0;
    if (i != optional) short_word[short_count++] = word[i];
  }
  if (optional < count) {
    for (size_t i = 0; i < count; ++i) {
      if (i != optional && word[i] == word[optional]) return 0;
    }
  }
  for (size_t i = 0; i < graph->reduction_count; ++i) {
    const TsguReduction reduction = graph->reductions[i];
    if (reduction.parent != parent) continue;
    if (reduction.width != count &&
        (optional == count || reduction.width != count - 1)) return 0;
    for (size_t child = 0; child < language->max_alias_sequence_length; ++child) {
      if (language->alias_sequences[(size_t)reduction.production *
                                   language->max_alias_sequence_length + child]) return 0;
    }
  }
  return tsgu_required_word(graph, parent, word, count) &&
    (optional == count || tsgu_required_word(graph, parent, short_word, short_count));
}

/* Unlike an exact word, a prefix must be checked at every selected reduction
 * width, including the entire unconstrained suffix path. A prefix-only walk
 * would confuse intermediate states with complete reductions. All widths
 * share one work budget; exhausting it refuses, never samples productions. */
TSGU_METADATA_EXPORT int TSGU_REQUIRED_PREFIX(const void *raw, const char *parent_name,
                                              const char *const *children, size_t count) {
  const TsguRequiredGraph *graph = raw;
  if (!graph || !children || !count || count > TSGU_MAX_WORD) return 0;
  const TSLanguage *language = graph->language;
  TSSymbol parent, word[TSGU_MAX_WORD];
  if (!tsgu_required_symbol(language, parent_name, &parent) ||
      parent < language->token_count || parent == graph->root) return 0;
  for (size_t i = 0; i < count; ++i) {
    if (!tsgu_required_symbol(language, children[i], &word[i])) return 0;
  }
  bool widths[UINT8_MAX + 1] = {false};
  size_t population = 0;
  size_t max_width = 0;
  for (size_t i = 0; i < graph->reduction_count; ++i) {
    const TsguReduction reduction = graph->reductions[i];
    if (reduction.parent != parent) continue;
    if (reduction.width < count) return 0;
    ++population;
    widths[reduction.width] = true;
    if (reduction.width > max_width) max_width = reduction.width;
    for (size_t child = 0; child < language->max_alias_sequence_length; ++child) {
      if (language->alias_sequences[(size_t)reduction.production *
                                   language->max_alias_sequence_length + child]) return 0;
    }
  }
  if (!population) return 0;
  size_t budget = TSGU_MAX_PATH_VISITS;
  for (size_t width = count; width <= max_width; ++width) {
    if (!widths[width]) continue;
    if (graph->reduction_count > budget) return 0;
    budget -= graph->reduction_count;
    if (graph->edge_count > budget / width) return 0;
    budget -= graph->edge_count * width;
    if (!tsgu_required_path_prefix(graph, parent, word, count, width)) return 0;
  }
  return 1;
}
