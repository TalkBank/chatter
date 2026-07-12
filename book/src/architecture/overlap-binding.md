# Overlap Marker Binding

**Status:** Current
**Last modified:** 2026-07-11 12:45 EDT

Overlap markers (`⌈` `⌉` `⌊` `⌋`) mark simultaneous speech. They are
the hardest tokenization problem in CHAT, because they legitimately
live at two levels: **between** words (marking a span boundary in the
utterance) and **inside** words (marking that the overlap boundary
falls mid-word, as in `o⌈ne t⌉wo`). This page documents the binding
rule the grammar ships (the whitespace-boundary custody rule, adopted
2026-07-11), the historical adjacency rule it replaced, and the
engineering record of how the replacement was proven safe. Read it
before touching `word_body`, `contents`, or anything overlap-adjacent.

## The shipping rule: whitespace-boundary custody

**A marker whose outer side touches whitespace (or the utterance edge)
is top-level content; a marker glued to word material on both sides is
word-internal.** Glued runs of structural marks (CA elements and
delimiters, underline marks, lengthening colons) share custody with
the marker they are glued to: every mark in a glued run has a glued
neighbour, so the whole run binds into the word.

```
Yeah ⌈2 hey     ⌈2 is standalone (whitespace both sides)
⌈one two⌉       (⌈) (one) (two) (⌉): edge markers are top-level
o⌈ne t⌉wo       TWO words with interior markers (glued both sides)
the⌉:           ONE word: the colon is prosodic lengthening, glued
                to the marker, which is glued to the word
Joo␂␁:␂␂⌉       ONE word: an underline-led glued trailing run
⌉↘              TWO items: intonation arrows are utterance-level
                separators even when glued (adjudicated 2026-07-11)
```

Span pairing (which `⌈` matches which `⌉`, top-with-bottom,
index-aware) is **model-derived**, not grammatical: unpaired markers
are common in real data and pairing is ultimately cross-speaker, so
the grammar states structure truthfully and `talkbank-model` derives
the relations (`extract_overlap_info`, `OverlapGroup`, E347/E348/E373).

Mechanically, the rule is carried by three weighted hidden rules in
`word_body`: `_interior_overlap` (glued mark(s) with immediate spoken
text after), `_final_overlap_cluster` (a trailing glued run: two arms,
overlap-led requiring a non-empty tail so a bare trailing `⌉` stays
top-level, and non-overlap-mark-led of any length), and
`_final_overlap_form` (glued run ending in a special-form marker).
All three carry `prec.dynamic(2)` **inside their rule bodies**, and
declared GLR conflicts let the fused and fragmented readings race;
the weight settles every race in favor of fusion.

### Two tree-sitter lessons (hard-won; do not rediscover)

1. **`prec.dynamic` attaches ONLY on seq-bodied rule productions.** It
   silently no-ops when wrapped around a symbol reference at a use
   site (`prec.dynamic(2, $._rule)` inside `repeat(choice(...))`) and
   when wrapped around a hidden choice-of-symbols rule body. Both were
   disproven empirically (2026-07-11): the generated `grammar.json`
   carries the metadata, but runtime subtree sums never see it, and
   GLR falls back to `select_earlier` (version order), whose winner
   flips with utterance position. Any behavior attributed to a
   non-attaching `prec.dynamic` is actually stack order.
2. **Verify forks in the `--debug` trace, not by reasoning.** The
   campaign's worst detours came from assuming a fork existed (or
   didn't) at a given token. The trace's `version_count`,
   `detect_error`, `condense`, and `select_*` lines are the ground
   truth about what the GLR runtime actually explored.

## The historical rule: adjacency bound into the word

Until 2026-07-11 the grammar shipped the **adjacency rule**: a marker
adjacent to text was part of the word; only a space-separated marker
was standalone. `⌈one two⌉` parsed as two words `⌈one` and `two⌉`.
The rule existed because the project's ORIGINAL ideal (the
whitespace-boundary rule above) was assessed in January 2026 as
requiring "bidirectional context that LR parsers cannot naturally
handle", and every grammar generation since carried the tractable
approximation forward.

## How the ideal was proven shippable (2026-07-10/11 campaign)

A feasibility experiment (2026-07-10) showed the ideal rule was
expressible with GLR machinery; a follow-up campaign (2026-07-11)
drove it to production quality. The evidence trail, in order:

- Interior fusion (`be⌉gin` one word) at zero regressions.
- The B4 bug family (`word⌉:` fragmenting at non-initial utterance
  positions) root-caused to the `prec.dynamic` attachment traps above;
  fixed by moving weights into rule bodies.
- Marker-led fusion generalized: any glued structural mark (underline,
  CA marks, not just overlap points) can lead a weighted interior run
  or trailing cluster, expressing the whitespace-boundary principle
  structurally.
- Final acceptance: 763 overlap-bearing files + 500 controls with
  ZERO regressions and 100% of remaining AST drift in the designed
  custody classes; a whole-corpus sweep (114,804 kept `.cha` files)
  with zero ERROR-parse files under either grammar.

The custody semantics (which glued shapes fuse, cell by cell) were
adjudicated in a juxtaposition matrix against wild-data evidence, with
the maintainer ruling directly on the contested cells (`the⌉:` is one
word; glued arrows are separators).

## Consumers and the model boundary

The typed model consumes both marker positions natively: contents-level
`overlap_point` items lower via `parse_overlap_point`, in-word markers
via the word-child dispatch (order-agnostic), so the custody change is
distributional, not structural, at the model seam. Span semantics
(pairing, groups, cross-speaker balance) live in
`talkbank-model::alignment::helpers::overlap` and
`overlap_groups`, and validation asserts kind- and index-aware
balance (E347/E348/E373, E704). For new transcription, note that CA
overlap markers are legacy notation: the preferred modern indication
of overlap is `&*OTH:` other-speaker events plus forced-alignment time
bullets; the grammar supports the markers indefinitely for the large
legacy CA corpora.

## Why this interacts with whitespace separation

The grammar's deepest design commitment is `extras: []`: all
whitespace is grammar-visible, because the worst historical CHAT
parser bug was ACCEPTING glued content items as if properly separated
(the legacy Java implementation tokenized `hello(.)` correctly as a
word and a pause, and that silent acceptance was precisely the
problem: malformed sources never got cleaned). The whitespace-boundary
custody rule is this same commitment applied to overlap markers: the
grammar never invents a word boundary where the transcriber wrote
none, and never erases one they wrote. Whitespace-separation
violations that the grammar tolerates for recovery's sake are rejected
by validation with precise diagnostics (E749, E750, E751), per the
layer rule: the grammar's job is SHAPE (parse everything, truest
tree); rejection of recoverable style belongs to validation, where
messages are helpful and recovery graceful.
