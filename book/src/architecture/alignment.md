# Alignment

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

Alignment in the toolchain operates at two structural layers, plus a
separate overlap-marker pass. Tier alignment is structural (counting and
pairing AST nodes); word extraction is positional (domain-ordered token
indices).

| Layer | Where | Purpose |
|---|---|---|
| **Tier alignment** | `talkbank-model::alignment` | 1:1 mapping between main tier and dependent tiers (`%mor`, `%pho`, `%wor`, `%sin`, `%gra`) |
| **Word extraction** | `talkbank-transform::extract` | Pull NLP-ready words from the AST in domain order |

## Tier Alignment

Validates that dependent tiers have the correct number and arrangement
of items relative to the main tier. Lives in
`crates/talkbank-model/src/alignment/`.

### TierDomain

```rust
enum TierDomain { Mor, Pho, Sin, Wor }
```

The same utterance produces different counts per domain:

| Rule | Mor | Pho | Sin | Wor |
|---|---|---|---|---|
| Skip retrace groups | Yes | No | No | No |
| Count pauses | No | Yes | No | No |
| PhoGroup | Recurse | Atomic (1) | Skip (0) | Recurse |
| SinGroup | Recurse | Skip (0) | Atomic (1) | Recurse |
| Include fragments (`&+`) | No | Yes | Yes | No |
| Include nonwords (`&~`) | No | Yes | Yes | No |
| Include fillers (`&-`) | No | Yes | Yes | Yes |
| Include untranscribed | No | Yes | Yes | No |
| Include tag-marker separators | Yes | No | No | No |
| `ReplacedWord` aligns to | Replacement | Original | Original | Original |

For the underlying word filter (`counts_for_tier`,
`should_skip_group`), the content walker, and the ChatFile model itself,
see [CHAT Data Model](chat-model/chat-model.md). The walker plus the
domain table together govern every tier-alignment count.

### Retrace handling, alignment-critical

Retraces are the most alignment-critical content type. A `Retrace` node
wraps content the speaker said then corrected.

- **Mor:** skip entirely (count `0`). The retrace was a false start;
  only the correction carries morphological analysis.
- **Pho, Sin:** recurse, words were physically produced and have
  phonological / gestural data.
- **Wor:** recurse, retrace ancestry does not change `%wor` membership.

**Critical invariant:** the parser must emit `UtteranceContent::Retrace`
for *all* retrace patterns, including single-word retraces with
replacements (`word [: repl] [* err] [//]`). If a retrace is
accidentally emitted as a bare `ReplacedWord`, it counts for `%mor`
alignment, causing false E705 errors. Enforced by
`tests/retrace_replaced_word_regression.rs`. Full data model + parsing
pipeline + CHAT examples in
[Retraces and Repetitions](../chat-format/retraces.md).

### AlignmentPair

```rust
struct AlignmentPair {
    source_index: Option<usize>,
    target_index: Option<usize>,
}
```

Universal index-pair primitive. `Some`/`Some` = matched. One `None` =
insertion / deletion placeholder for mismatch diagnostics.
`is_complete()`, both indices `Some`. `is_placeholder()`, unmatched.

### Per-domain results

| Type | Function | Source → Target |
|---|---|---|
| `MorAlignment` | `align_main_to_mor()` | Main → `%mor` items |
| `PhoAlignment` | `align_main_to_pho()` | Main → `%pho` tokens |
| `SinAlignment` | `align_main_to_sin()` | Main → `%sin` tokens |
| `WorAlignment` | `align_main_to_wor()` | Main → `%wor` tokens |
| `GraAlignment` | `align_mor_to_gra()` | `%mor` chunks → `%gra` relations |

`%gra` aligns to `%mor` *chunks*, not items. Clitics create additional
chunks (`pro|it~v|be&PRES` = 2 chunks: pre-clitic + main).

### Trait abstractions

| Trait | Purpose | Implementors |
|---|---|---|
| `IndexPair` | `source()`/`target()` on any pair type | `AlignmentPair`, `GraAlignmentPair` |
| `TierAlignmentResult` | `pairs()`/`errors()`/`push_*()` accumulator | All 5 alignment result types |
| `AlignableTier` | What a tier provides for generic alignment | `PhoTier`, `SinTier`, `WorTier` |
| `TierCountable` | `count_tier_positions()` / `collect_tier_items()` methods | `[UtteranceContent]` |

The generic `positional_align()` function uses `AlignableTier` to
eliminate duplication: `align_main_to_{pho,sin,wor}()` are thin
wrappers around it. `%mor` doesn't use it (additional terminator
validation logic). `%gra` doesn't use it (source is `MorTier`, not
`MainTier`). `WorTier` overrides `mismatch_format()` to `Diff` (LCS) since
both sides are word sequences; the others use `Positional`.

### `%wor` is not validated

`%wor` is a timing-annotation tier. There is no downstream positional
indexing into `%wor`, and `validate_alignments()` does **not** check
`%wor` word count against the main tier. Old corpus files may have
`xxx`, fragments, or nonwords in `%wor` (pre-2026-04 behavior) without
producing false errors.

### Phon tier-to-tier alignment

A second class of alignment that operates **between dependent tiers**:

| Source | Target | Code |
|---|---|---|
| `%modsyl` | `%mod` | E725 |
| `%phosyl` | `%pho` | E726 |
| `%phoaln` | `%mod` | E727 |
| `%phoaln` | `%pho` | E728 |

Derived-view alignments: `%modsyl` is a syllabified reannotation of
`%mod`, `%phosyl` of `%pho`, `%phoaln` aligns both. Word counts must
match between source and target. Computed in `compute_alignments()`
after the main-tier alignments. `build_tier_to_tier_alignment()`
constructs index pairs and emits `build_count_mismatch_error()` when
counts disagree. `%phoaln` checks against both `%mod` and `%pho`,
potentially emitting E727 and E728 simultaneously.

**Known data issue:** Phon XML source data has orthography↔IPA word
count discrepancies in ~4% of files (518 / 12,340). Expected in child
phonology data. The PhonTalk converter handles this inconsistently,
`%mod`/`%pho` are truncated to match orthography via `OneToOne`, but
`%xmodsyl`/`%xphosyl`/`%xphoaln` are written from raw `IPATranscript`,
exposing the full IPA word count. Result: E725-E728 mismatches.

### Parse-health gating

Alignment diagnostics honor `ParseHealth` metadata. If a dependent
tier's domain is parse-tainted, mismatch errors for that domain pair
are suppressed. Main-tier taint blocks all main→dependent alignments.
Dependent-tier taint blocks only that tier. Phon tier-to-tier checks
have their own gates (`can_align_modsyl_to_mod`,
`can_align_phosyl_to_pho`, `can_align_phoaln`).

## Word Extraction

`extract_words()` (in `crates/talkbank-transform/src/extract.rs`) uses
the content walker to pull words from the AST in domain-specific order.
Returns `Vec<ExtractedWord>` with `text`, `word_index`, `is_separator`,
`special_form`. Tag-marker separators (`,` `„` `‡`) are included as
words in Mor domain because they have `%mor` items (`cm|cm`,
`end|end`, `beg|beg`).

## Overlap Marker Iteration

CA overlap markers (⌈⌉⌊⌋) appear at three content levels,
`UtteranceContent` (top-level), `BracketedItem` (inside groups), and
`WordContent` (intra-word, `butt⌈er⌉`). Two APIs in
`talkbank-model/src/alignment/helpers/overlap.rs`:

### `walk_overlap_points`, low-level

Visits every `OverlapPoint` in document order with word-position
context. Analogous to `walk_words` but for overlap markers:

```text
walk_overlap_points(&utterance.main.content.content.0, &mut |visit| {
    // visit.point: &OverlapPoint (kind + optional index)
    // visit.word_position: usize (alignable words seen so far)
});
```

### `extract_overlap_info`, region-based

Pairs markers by (kind, index) into `OverlapRegion` structs. Each
region represents a matched ⌈...⌉ or ⌊...⌋ pair. Index-aware:
`⌈2...⌉2` forms a separate region from `⌈...⌉`. Mismatched indices
leave markers unpaired. Onset-only ⌈ (without ⌉) is a legitimate CA
convention, region has `end_at_word = None`,
`is_well_paired() = false`, but `top_onset_fraction()` still works.

### Cross-utterance, `analyze_file_overlaps`

For whole-file analysis, in `overlap_groups.rs`. 1:N matching: one
top region from speaker A can match multiple bottom regions from
speakers B, C, etc. Used by E347 and `chatter debug overlap-audit`.

### Overlap validation

| Code | Level | Check |
|---|---|---|
| E347 | Cross-utterance | Orphaned tops/bottoms with 1:N matching (warning) |
| E348 | Utterance | Unpaired markers within a single utterance (warning) |
| E373 | Utterance | Invalid overlap index values (must be 2-9) |
| E704 | Cross-utterance | Same speaker encoding both top and bottom (error) |

`chatter debug overlap-audit <path>` reports per-file statistics
(groups, bottoms, orphans, temporal consistency) in TSV format. Use
`--database <path.jsonl>` for a persistent JSON-lines database.

## Design Principles

1. **No string hacking.** All alignment operates on typed AST
   structures (`Word`, `MorTier`, `AlignmentPair`), never on serialized
   CHAT text.
2. **Domain-aware from the start.** `TierDomain` gates traversal at the
   walker level. Downstream code never re-implements retrace / group
   skipping logic.
3. **Deterministic over approximate.** Tier alignment and word
   extraction use deterministic, positional algorithms over the typed
   AST.
4. **Dense indexed structures.** `AlignmentPair` uses `Option<usize>`
   rather than cloned data; index pairs are stored positionally, not in
   hash maps.
5. **Exhaustive matching.** Every `match` on `UtteranceContent` (24
   variants) or `BracketedItem` (22 variants) lists all variants
   explicitly. New variants are a compile error, not a silent bug.
6. **Walker as shared primitive.** `walk_words()` removed ~330 lines of
   duplicated traversal boilerplate across 7 call sites.

## Downstream Consumers

| Consumer | Crate | Usage |
|---|---|---|
| Validation | `talkbank-model` | Cross-tier checks (E714/E715, E725-E728), overlap (E347/E348/E373/E704) |
| LSP hover | `talkbank-lsp` | Show aligned tier items for word under cursor |
| Word extraction | `talkbank-transform` | NLP-ready words from utterances |
| Overlap audit | `chatter` | `chatter debug overlap-audit` |
| `%wor` generation | `talkbank-model` | Build `%wor` tier from main tier |
