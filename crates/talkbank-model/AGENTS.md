# Model crate agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-model/AGENTS.md)

Read the [root guidance](../../AGENTS.md) first. This file adds the model's
ownership and boundary rules; it does not create separate gates.

## What this crate is

The typed CHAT AST (`model/`), its validation (`validation/`), cross-tier
alignment (`alignment/`), the diagnostic types (`errors/`), `WriteChat`
serialization and the `ChatParser` trait (`parser_api/`). It depends on no
parser. Both backends produce its values; every downstream crate reads them.
Tree-sitter and re2c are peers and can both drive model changes, including
malformed/recovery states. Follow
[shared model ownership](../../book/src/architecture/parser-backends.md#equal-backends-one-shared-model);
do not treat Tree-sitter's current lowering as a fixed model boundary.
Architecture: [CHAT data model](../../book/src/architecture/chat-model/chat-model.md),
[errors](../../book/src/architecture/errors-and-validation/chat-core-errors.md),
[validation](../../book/src/architecture/errors-and-validation/validation.md),
[alignment](../../book/src/architecture/alignment.md).

## Construction

- No silent defaults or sentinels in model data, ever. Preserve observed
  missing/malformed states explicitly; never invent a normal payload to make a
  constructor succeed. That causes spurious diagnostic cascades and silent
  corruption. Follow the
  [design principle](../../book/src/architecture/chat-model/chat-model.md#no-silent-defaults-or-sentinels).
- Every value with a source position carries `location: Location`
  (`Located(Span)` or `Unlocated`) and takes it as its constructor's first
  argument. There is no setter, `Default` or placeholder span. A builder
  writes `Location::Unlocated`; a parser passes its node's span. The
  serde default `Location::unlocated` exists only because located fields
  leave the AST wire and come back unlocated.
- `Span` is geometry (`start <= end`, asserted in every profile). Trusted
  producers use `Span::new`; untrusted offsets go through `Span::try_new` or
  `TryFrom<Range<usize>>`. Combine locations with `Location::or`, `map`,
  `start_point` and `span()`; a consumer that needs geometry matches and
  states its unlocated policy.
- Distinguish admitted `NonEmptyString` from `RecoveryText`, whose `Empty`
  variant preserves observed emptiness for contextual validation. Never infer
  admission from a constructor name: inspect serde and recovery paths too. `new_unchecked` and
  `from_checked_literal` are counted by the fabricated-AST ratchet; adding one
  is a reviewed exception, never a convenience.
- `ChatFile` is the mutable AST; `validate_into` consumes it into a read-only
  `ValidChatFile`. Nothing deserializes into the accepted type and nothing
  edits it without `into_unchecked()`. JSON import does not reconstruct parser
  trust (E251 is checked at the model boundary for exactly that reason).

## Validation

- Presence decides, location points. A rule fires because the construct is in
  the model, never because it has a span; the span only chooses where the
  diagnostic points. A value with no location of its own (header payloads,
  leaf tokens) reports at `ValidationContext::field_diagnostic_location()`,
  which its parent narrowed with `with_field_location`; `Unlocated` is the
  honest answer when nothing narrowed, never `at_offset(0)`.
- Two facts, never one: `ParseHealth` taint answers whether cross-tier
  alignment is trustworthy; a tier's own failed lowering remains explicit
  rejection or an unparsed variant. GraLowering never constructs a partial
  GraTier. Recovery is not validity.
- Exhaustive matches over `UtteranceContent` and `BracketedItem`; "consecutive"
  on the main tier is `walk_words` order, never flat-index adjacency.
- Fabricated values are banned: no `_ => Separator::Comma`, no
  `unwrap_or("")`, no `Default` whose wrong value is invisible. Say unknown or
  invalid with a variant or a `Result`.

## Owners to find before writing

- Index spaces: `alignment/indices.rs` (the worked typestate graph). MOR
  items, MOR chunks, main words, phone items and authored GRA indices are not
  interchangeable; `GraHeadRef` distinguishes ROOT from a word reference.
- Pairing, counting or aligning two tiers: `WorMainTierProjection`,
  `AlignmentSet`, `MorTier::chunks()`. Travel the owner to its last verdict
  before adding a step beside it.
- Closed vocabularies (symbols, form markers, error codes) are generated under
  `generated/` from `spec/`; never hand-write a list of a closed set here.
- `errors/location.rs`, `errors/source_location.rs` and `errors/span_shift.rs`
  own `Location`, `DiagnosticLocation` and how both move after an edit.

## Rendering boundary

`WriteChat` implementations are generic over `ChatWriter` and return the sink's
associated error. Emit text, characters and decimal integers directly; do not
route through arbitrary `Display` formatting. Owned-string helpers use
`ChatString::render`, whose error type is `Infallible`; never discard a
`fmt::Result` or replace it with a panic. Existing `std::fmt::Write` sinks remain
supported and their errors must propagate. Rendering does not certify validity.

## Testing boundary

This crate cannot parse, so a test that needs a `ChatFile` built from CHAT
text belongs in `talkbank-parser-tests`, not here: a hand-built AST states
the input twice and can describe a shape no parser produces. Keep here the
tests of validating constructors, rejected raw inputs, wire formats,
`SpanShift`, `WriteChat` roundtrips and validation policy over values the
constructors can legitimately build. `cargo test -p talkbank-model --lib` is
the focused loop; schema doc comments regenerate `schema/chat-file.schema.json`
through `just schema-gen`.
