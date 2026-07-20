# The CHAT recovery and leniency boundary: where "parse, don't validate" sits

**Status:** Reference
**Last modified:** 2026-07-18 22:12 EDT

This is a decision record for a boundary that has governed the parser and
model since the beginning but was never written down: **how lenient is the
parser, and where does the line between recovery and rejection fall?** It
matters most to a contributor who inherits this code and wonders why an
orphaned dependent tier is dropped while a leading space is faithfully
preserved and flagged. The short answer is that the boundary is drawn by
the error taxonomy of real CHAT data, not by philosophical purity.

## The principle we follow, and where it bends

`chatter` follows **parse, don't validate**: the parser represents what is
actually in the source and defers judgments of validity to the validator,
so that no information is silently discarded and the validator (not the
parser) decides what is legal. The grammar's "strict + catch-all" pattern
is the everyday expression of this: unknown option names, media types, and
the like parse into a generic node and are flagged later, never rejected at
parse time.

But the principle is not absolute, and it is honest to say where it bends.
`Line::Utterance(Utterance)` (see `crates/talkbank-model/src/model/file/line.rs`)
bundles a main tier together with the dependent tiers that belong to it.
That grouping is a **parse-time structural commitment**: the parser decides,
while parsing, which `%`-tiers attach to which `*`-tier. A source that
violates the grouping (a `%mor` before any `*` line, a `@Comment` wedged
between a main tier and its `%mor`) cannot be grouped, and the parser
**rejects** it rather than representing it:

- an orphan dependent tier yields **E404** ("Dependent tier appears before
  any main tier") and the tier is **dropped from the model** (the recovery
  path `report_top_level_dependent_tier_error` in
  `crates/talkbank-parser/src/parser/chat_file_parser/chat_file/helpers.rs`
  takes `lines: &mut [Line]` -- a slice it cannot even push onto -- so it
  only emits a diagnostic and marks taint);
- a dependent tier separated from its main tier by an interleaved header
  yields **E600** and fails to attach.

So for this one invariant -- "dependent tiers belong to the main tier above
them" -- the parser is stricter than pure parse-don't-validate would be. A
purist model would represent every physical line independently, in file
order, and let the validator report the orphan while still showing its
content.

## Why the boundary is drawn here: the error taxonomy of real data

The boundary is deliberate, and it is drawn from what people actually type,
not from what is theoretically expressible. CHAT errors fall into two
classes, and the parser treats them differently on purpose.

### Class 1: line-level malformations people make and expect recovery from

These are typos and slips *within* a well-placed line: a leading space
after the tab (E758), a missing terminator (E305), a glued pause or code
(E751 / E757), a comma without spacing (E258 / E259), a malformed token.
People produce these constantly, and a good tool must **recover**: it
represents the line faithfully (the recovered structure survives in the
model, with real source spans) and the validator flags the problem. The
whole-tree recovery backstop and the typed `NodeSlot` handling exist for
exactly this: a malformed line is preserved, never silently swallowed.

The E758 separator work is a textbook Class-1 case. A trailing space in a
tier separator is now grabbed by a dedicated `sep_trailing_space` node,
kept as separator provenance (a `TierSeparator`), flagged as E758 in a
non-CA file, and canonicalized away on roundtrip. The line is represented;
the mistake is reported; nothing is lost.

### Class 2: gross structural violations people essentially never make

These are dislocations of whole lines: a dependent tier floating with no
main tier, a `%`-tier appearing out of any sensible context, arbitrary
interleavings that no transcription workflow produces. **These do not occur
in real corpora.** Transcribers write a main tier and then its dependent
tiers directly beneath it; morphological tools emit `%mor`/`%gra` attached
to their utterance; nobody commits a stray, context-free `%mor`.

For Class 2 the parser makes its structural commitment and **rejects** (the
E404/E600 behavior above). This is not a failure of the philosophy; it is
the philosophy correctly weighing cost against reality:

- **Faithfully representing Class 2 has near-zero practical value**, because
  the inputs it would preserve do not exist in the data we serve.
- **The cost of representing it is the largest refactor in the model**:
  `Utterance` -- the spine of alignment, morphology, and nearly every
  downstream analysis -- would have to become a *derived, validated view*
  over an ungrouped sequence of source lines, and E404/E600 would move from
  parse-time to validation-time. Every consumer of `main` and
  `dependent_tiers` would change.
- **A loud rejection is the right outcome anyway.** When a file does contain
  a dislocated tier, it is broken, and the researcher should be told
  clearly (E404/E600), not handed a tool that lovingly preserves the
  nonsense and buries the signal.

So the boundary sits between the two classes: **line-level well-formedness
is recovered and represented; cross-line grouping (dependent tiers belong
to their main tier) is a parse-time commitment enforced by rejection.**
That is the honest, defensible place for the `parse, don't validate` line
for CHAT, precisely because the grouping invariant holds in all real data.

## When to revisit this

Reopen the decision only if real corpora start exhibiting Class-2 structure
that must be preserved rather than rejected -- for example if an upstream
tool legitimately emits ungrouped or interleaved tiers that TalkBank must
ingest without loss. At that point the source-line decomposition (each
physical line a `Line` variant; `Utterance` a derived grouping) becomes
worth its cost, and this record is the starting point for that redesign.

The one improvement worth considering short of the full decomposition, and
strictly lower priority, is **diagnostic quality** for the rare Class-2
case: surfacing the dropped tier's content in the diagnostic so a user sees
what was rejected, rather than only that something was. That does not
require ungrouping; it is a message-fidelity change in the recovery path.

## Related

- CHAT line model: `crates/talkbank-model/src/model/file/line.rs`
- Recovery path for orphan/interleaved tiers:
  `crates/talkbank-parser/src/parser/chat_file_parser/chat_file/helpers.rs`
- Recovery is not validity, whole-tree backstop, typed `NodeSlot` handling:
  the root `CLAUDE.md` "Parser Recovery and Data Integrity" and "CST
  Traversal Rules" sections.
- The E758 separator work (a Class-1 recovery) and the `TierSeparator`
  model: `docs/proposals/2026-07-18-source-spacing-ground-truth.md`.
