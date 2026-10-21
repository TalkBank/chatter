# Overlap Marker Binding

**Status:** Current
**Last modified:** {{git-dates:page}}

Overlap markers (`⌈` `⌉` `⌊` `⌋`) mark simultaneous speech. They are
one of CHAT's tokenization ambiguities, because they legitimately
live at two levels: **between** words (marking a span boundary in the
utterance) and **inside** words (marking that the overlap boundary
falls mid-word, as in `o⌈ne t⌉wo`). This page documents the binding
rule the grammar ships and a historical alternative. The alternative and
its experimental measurements are not the current parser contract or an
approved migration plan. Read the ownership distinction before changing
`word_body`, `contents`, or overlap conversion.

## The shipping rule: adjacency binds into the word

**A marker adjacent to text is part of the word; only a
space-separated marker can be a standalone `overlap_point`.** These examples
describe content shapes, not complete valid transcripts.

```
Yeah ⌈2 hey     ⌈2 is standalone (spaces both sides)
⌈one two⌉       TWO words: "⌈one" and "two⌉" (markers bound in)
o⌈ne t⌉wo       TWO words with interior markers (same rule)
```

Mechanically, the structured `word_body` rules bind adjacent markers as
`overlap_point` children. The marker does not become part of a `word_segment`;
the generated segment exclusions preserve that distinction. Word ownership
and token kind are separate facts. See
[Word Internals](../chat-format/word-internals.md#1-overlap-markers-⌈⌉⌊⌋)
and the grammar's `grammar/docs/tokenization-rules.md` (Exception 1).

The marker-initial branch still requires a `word_segment`, `shortening` or
`stress_marker` after its leading markers. A contiguous marker-only run does
not become a word merely because there are no spaces. The `overlap_point`
token accepts one of the four brackets with an optional single digit `1` to `9`.

Conversion preserves the structural owner: `push_piece` in the tree parser's
`word/pieces.rs` constructs `WordContent::OverlapPoint`, while the contents
walker constructs `UtteranceContent::OverlapPoint` for standalone markers.
Both use the shared overlap-token decoder. Lexical `cleaned_text()` excludes
the structural marker and its number; the model retains the marker for CHAT
serialization. Neither binding nor lexical projection proves paired-marker
validity, cross-speaker correspondence or acoustic timing.

## Historical alternative: edge markers as content

**A marker with spoken text on BOTH sides is word-internal; a marker
at a word's edge is top-level content.** Under this proposed rule,
`⌈one two⌉` parses as `(⌈) (one) (two) (⌉)`: the visually obvious
reading: while `o⌈ne` keeps its interior marker. The shipping rule
diverges exactly at word edges, where it gives the word custody of
markers the ideal calls top-level.

The earlier design record described this as the intended rule. Early prototype
grammars (January 2026) attempted it and produced a substantial
decision record; the analysis concluded that the rule "requires
bidirectional context that LR parsers cannot naturally handle" (the
parser must see both sides of the marker to classify it, and LR(1) has
one token of lookahead). The adjacency rule was adopted as the
tractable alternative. That historical explanation is not a proof that the
alternative is impossible; the later experiment below explored another route.

## Historical experiment (2026-07-10)

A feasibility experiment revisited the impossibility conclusion with
GLR machinery the January analysis had not combined: an interior-only
`word_body` (a word may not begin or end with an overlap marker), a
declared conflict, dynamic precedence on interior continuations, and
removal of the static `prec.right` bias so the conflict genuinely
splits. The retained narrative reports these results; the counts below are
historical observations, not a current-source corpus acceptance result:

- The ideal rule IS expressible: probes and grammar fixtures parse to
  the ideal shapes with no ERROR nodes, and the grammar's conflict
  inventory net-shrinks.
- **Corpus reality is the hard part.** Conversation-analysis (CA)
  transcription layers: overlap points, paired CA delimiters,
  underline spans, lengthening, compounds: cross-nest freely at word
  edges (`☺you ⌈there⌉☺`, `∇⌈ho:ney⌉∇`, `full⌉+grown`,
  `⌈drug⌉ [!]`). The shipping rule sidesteps every such case by
  giving the word custody of everything adjacent; the ideal rule must
  answer a custody question PER MARKER PAIR, each answer costing a
  grammar rule, a conflict, and an AST-shape decision. Measured
  against the full kept corpus (763 overlap-bearing files, all of
  which parse cleanly under the shipping grammar), five iterations of
  custody rules reduced ideal-rule regressions from 195 files to 105:
  a converging but long tail.

The experiment proposed two implementation routes:

1. **Grammar route**: finish the custody enumeration, followed by coordinated
   migration of the model, generated traversal and re2c backend. The old
   multi-week estimate is not a current implementation schedule.
2. **Conversion route (recommended by the experiment)**: keep the
   shipping grammar, and re-associate edge-bound overlap points to top
   level during CST-to-model conversion. At that point the CA layers
   are already resolved into typed word children, so every custody
   question becomes a deterministic tree transformation rather than a
   GLR ambiguity. The original proposal cited CA terminator promotion as a
   precedent, but that is **not current behavior**: CA arrows and TCU markers
   remain separators or linkers, not promoted terminators. See
   [CA Terminator Resolution](parser-and-grammar/ca-terminator-resolution.md).
   The grammar's
   empty-extras design (all whitespace grammar-visible) preserves
   exactly the facts the transformation needs.

Neither proposal changes the shipping ownership rule documented above.
Adopting either would require a new reviewed contract, generated-artifact and
backend reconciliation, serialization/lexical controls, and source-bound
evidence. Do not treat the historical recommendation as authority to normalize
edge markers during conversion.

## Why this interacts with whitespace separation

The grammar uses `extras: []`: whitespace is explicit rather than silently
skipped. This does not make whitespace a universal tokenizer or guarantee that
every malformed input has a complete recovered model. Adjacent notation has
construct-specific rules; see the tokenization guide for overlap, suffix,
compound, shortening and other contexts.

Recovery and validity remain separate. The diagnostic owner depends on which
layer retains the needed evidence:

- E749 diagnoses a comma glued to the following word (`one ,two`) using
  in-order content adjacency. A comma attached on its left (`one, two`) is
  a different case.
- E751 diagnoses a pause glued to the preceding word (`hello(.)`), also using
  model spans and content adjacency.
- E750 diagnoses whitespace immediately inside angle-group delimiters
  (`< dog>` or `<dog >`) during parsing. The offending whitespace is not
  retained in the model, so this is not a later model-validation check.

The authored contracts in `spec/errors/E749.md`, `E750.md`,
`E750_group_edges.md` and `E751.md` own those boundaries. None authorizes a
second raw-text parser or makes a recovered AST valid by itself.
