# Overlap Marker Binding

**Status:** Current
**Last modified:** 2026-07-10 12:06 EDT

Overlap markers (`⌈` `⌉` `⌊` `⌋`) mark simultaneous speech. They are
the hardest tokenization problem in CHAT, because they legitimately
live at two levels: **between** words (marking a span boundary in the
utterance) and **inside** words (marking that the overlap boundary
falls mid-word, as in `o⌈ne t⌉wo`). This page documents the binding
rule the grammar ships, the ideal rule it approximates, why the gap
exists, and the measured options for closing it. It is the permanent
record of a design debate that has run since the project's earliest
prototypes; read it before touching `word_body`, `contents`, or
anything overlap-adjacent.

## The shipping rule: adjacency binds into the word

**A marker adjacent to text is part of the word; only a
space-separated marker is a standalone `overlap_point`.**

```
Yeah ⌈2 hey     ⌈2 is standalone (spaces both sides)
⌈one two⌉       TWO words: "⌈one" and "two⌉" (markers bound in)
o⌈ne t⌉wo       TWO words with interior markers (same rule)
```

Mechanically: `overlap_point` and `word_segment` carry equal token
precedence, and maximal munch plus `word_body`'s continuation rules
give the word custody of every adjacent marker. See
[Word Internals](../chat-format/word-internals.md) (tokenization
ambiguity #1) and the grammar's `tokenization-rules.md` (Exception 1).

## The ideal rule this approximates

**A marker with spoken text on BOTH sides is word-internal; a marker
at a word's edge is top-level content.** Under the ideal rule,
`⌈one two⌉` parses as `(⌈) (one) (two) (⌉)`: the visually obvious
reading: while `o⌈ne` keeps its interior marker. The shipping rule
diverges exactly at word edges, where it gives the word custody of
markers the ideal calls top-level.

The ideal was the project's ORIGINAL specification. Early prototype
grammars (January 2026) attempted it and produced a substantial
decision record; the analysis concluded that the rule "requires
bidirectional context that LR parsers cannot naturally handle" (the
parser must see both sides of the marker to classify it, and LR(1) has
one token of lookahead). The adjacency rule was adopted as the
tractable alternative, and every grammar generation since (through the
February coarsening campaign and the March re-structuring) has carried
it forward.

## What 2026-07-10 established

A feasibility experiment revisited the impossibility conclusion with
GLR machinery the January analysis had not combined: an interior-only
`word_body` (a word may not begin or end with an overlap marker), a
declared conflict, dynamic precedence on interior continuations, and
removal of the static `prec.right` bias so the conflict genuinely
splits. Results:

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

Two implementation routes therefore exist:

1. **Grammar route**: finish the custody enumeration. Honest estimate:
   a multi-week grammar project, followed by AST migration across the
   model, the generated visitor, the second (oracle) parser, and the
   XML emitter.
2. **Conversion route (recommended by the experiment)**: keep the
   shipping grammar, and re-associate edge-bound overlap points to top
   level during CST-to-model conversion. At that point the CA layers
   are already resolved into typed word children, so every custody
   question becomes a deterministic tree transformation rather than a
   GLR fight. Precedent: CA terminator promotion, which already uses
   this parse-one-way/normalize-at-conversion pattern. The grammar's
   empty-extras design (all whitespace grammar-visible) preserves
   exactly the facts the transformation needs.

The choice between routes (or deferral) is an open maintainer
decision at the time of writing; this page must be updated when it is
made.

## Why this interacts with whitespace separation

The grammar's deepest design commitment is `extras: []`: all
whitespace is grammar-visible, because the worst historical CHAT
parser bug was ACCEPTING glued content items as if properly separated
(the legacy Java implementation tokenized `hello(.)` correctly as a
word and a pause, and that silent acceptance was precisely the
problem: malformed sources never got cleaned). Overlap markers and a
short list of negotiated exceptions (notably comma-left: `one, two`
is accepted; `one ,two` is not) are the only constructs that
legitimately juxtapose with words at all. Whitespace-separation
violations that the grammar tolerates for recovery's sake are rejected
by validation with precise diagnostics (E749, E750, E751), per the
layer rule: the grammar's job is SHAPE (parse everything, truest
tree); rejection of recoverable style belongs to validation, where
messages are helpful and recovery graceful.
