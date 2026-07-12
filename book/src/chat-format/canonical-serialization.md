# Canonical Serialization

**Status:** Current
**Last modified:** 2026-07-11 16:02 EDT

This chapter is the formal definition of how chatter serializes a CHAT
model back to text: the **canonical form**. It exists because CHAT
source files vary freely in spacing while the typed model is
deliberately **spacing-agnostic**: between contents items, source
whitespace carries no meaning (custody of every marker is decided by
what it touches, not by how the transcriber spaced it). Serialization
therefore does not reproduce source bytes; it emits one well-defined
convention.

## The roundtrip invariant (semantic, not textual)

> **Invariant.** For every model `m`: `parse(serialize(m))` is
> semantically equal to `m` (`SemanticEq`).

Byte-identity with the *source* is deliberately NOT an invariant:
`who ⌈ is here ⌉ .` and `who ⌈is here⌉ .` are two
spellings of the same utterance and parse to the same model. What IS
guaranteed at the byte level is the **fixpoint property**:

> **Fixpoint.** Canonical text is stable under
> reparse-and-reserialize: `serialize(parse(t)) = t` whenever `t` is
> itself canonical output. Equivalently, canonicalization is
> idempotent.

Regression gates: the canonical-spacing unit tests
(`talkbank-parser`, `canonical_overlap_spacing`) pin the form; the
golden main-tier roundtrip harness (`talkbank-parser-tests`) asserts
the semantic invariant plus the fixpoint on every golden tier.

## Space conventions (main-tier contents)

The general rule, then its exceptions:

1. **Single-space join.** Adjacent contents items (words, groups,
   events, separators, quotations, retraces, ...) are joined by exactly
   one ASCII space.
2. **Tight overlap points** (ratified 2026-07-11): a contents-level
   OPENING overlap marker glues to the FOLLOWING item; a CLOSING marker
   glues to the PRECEDING item.

   ```text
   who ⌈is here⌉ ?        (canonical)
   who ⌈ is here ⌉ ?      (accepted input; canonicalizes to the above)
   ```

   Rationale: this is the dominant spelling in the wild CA corpora, so
   normalization does not churn real files, and it matches the visual
   semantics (the markers hug the overlapped speech).
3. **Comma glues left**: `one, two`. The grammar's negotiated
   comma exception accepts `one, two` and rejects `one ,two`; canonical
   form is the preferred glued spelling. All other separators are
   space-joined.
4. **Word-internal children are contiguous by definition.** Everything
   inside a `word_body` (segments, glued overlap runs, CA marks,
   lengthening, underline marks) serializes with no internal spaces:
   word custody IS contiguity.

## The rest of the tier

- **Linkers** are space-joined and precede the content.
- **Language code** (`[- eng]`) follows linkers, space-separated.
- **Terminator** is preceded by one space (`... word .`).
- **Postcodes** follow the terminator, each preceded by one space.
- **Terminal bullet** (time alignment) comes last, preceded by one
  space.

## What canonicalization may change in a real file

Running `chatter normalize` on wild data rewrites only spacing between
contents items, toward the conventions above. Content, custody, order,
annotations, and every non-main-tier byte are untouched. Because the
invariant is semantic, a normalize pass never changes what a file
*means*: and the fixpoint property means a second pass changes nothing
at all.

## History

Until 2026-07-11 the writer space-joined every contents item
uniformly, and byte-level roundtrip tests happened to pass because the
adjacency custody rule made glued edge markers word-internal (no
contents-level gluing existed to preserve). The whitespace-boundary
custody redesign surfaced the question; the maintainer ruled that the
roundtrip invariant is semantic, ratified the tight overlap form, and
this definition was written down. See
[Overlap Marker Binding](../architecture/overlap-binding.md) for the
custody design this builds on.
