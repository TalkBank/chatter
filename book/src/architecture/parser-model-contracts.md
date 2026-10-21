# Parser, Model, and API Contracts

**Status:** Current
**Last updated:** {{git-dates:page}}

This page describes the boundaries between source parsing, recovery, model
validation and serialization. For compiled examples, use
[Library Usage](../chatter/integrating/library-usage.md); for compatibility,
use [Versioning](../chatter/integrating/versioning.md). A current implementation
description is not a promise that every pre-1.0 API is stable.

## Parser entry points

`TreeSitterParser` is the primary parser handle. Reuse an instance when
processing multiple inputs. The native `talkbank-parser-re2c` backend is a
separate implementation; shared model types do not establish identical recovery,
diagnostics or complete behavioral equivalence. See
[parser backends](parser-backends.md) for supported comparisons and fragment
context.

The inherent `TreeSitterParser::parse_chat_file` method and the streaming
`ChatParser` trait have different result contracts:

| Boundary | Outcome | Diagnostics |
| --- | --- | --- |
| Inherent whole-file parse | `ParseProduct::Built { file, diagnostics }` or `Unbuildable { diagnostics }` | Retained in the product |
| Streaming/fragment parse | `ParseOutcome::Parsed(value)` or `Rejected` | Reported separately through the supplied `ErrorSink` |
| Required model admission | `ValidChatFile` or an explicit failure | Parsing and validation evidence must both be considered |

There is no `Success / Partial / Failed` status field in `ParseOutcome`.
A built model can still carry error diagnostics. Conversely, a rejected outcome
does not authorize discarding diagnostics already emitted to the sink.
For example, the trait word method is `parse_word`; the primary parser's
inherent helper is `parse_word_fragment`. Do not mix their signatures.

## Recovery is not admission

Recovery makes retained content available for inspection and editor diagnostics.
It does not make malformed source valid. A caller must not treat
`is_built()`, `is_parsed()`, or the absence of a panic as an admission proof.

`parse_validated_with_parser` requires validation and returns `ValidChatFile`
under the requested `ValidationPolicy`. Its admission boundary checks internal
failure evidence before severity filtering, then rejects parse errors or an
unbuildable product before model validation. Internal failure is not a verdict
that the transcript is invalid.

The optional-validation convenience APIs return a mutable `ChatFile` and may
skip checks according to their options. Do not substitute them for required
admission merely because both APIs can return a model.

The recovery design forbids inventing required text, words, enum choices or
timestamps to make construction succeed. That is a requirement to verify at
each producer/converter boundary, not a blanket claim that every backend
already satisfies it. Inspect actual retained model and serialized output in
regressions; an emitted error code alone cannot detect fabricated values.
Source diagnostics must describe the original input, not invented replacements.

## Source identity and coordinates

Keep the original UTF-8 source with the diagnostics that refer to it. Do not
normalize line endings, trim whitespace or rebuild source from a model before
using source byte offsets. Transformed text is a different coordinate space.

`parse_source_with_parser` binds the original source and its parse in
`ParsedSourceChat`. Calling `admit` consumes that state and checks complete
rules and tier alignment before returning `AdmittedSourceChat`, which couples
unchanged source bytes with an immutable admitted model. There is no public
constructor that combines arbitrary text with an unrelated model proof.

`into_product` releases recovery data without that admission authority;
`into_valid_file` relinquishes the source-byte binding. A transform that removes
and regenerates tiers uses its separate replacement admission, not unchanged
source preservation. See the [library guide](../chatter/integrating/library-usage.md).

For fragments, respect the API's source origin and contextual policy. A
document-relative offset is not an index into a fragment string. Legacy
synthetic wrappers and native fragments are not interchangeable evidence.
See the [diagnostic contract](../chatter/integrating/diagnostic-contract.md)
for checked presentation and client coordinate conversion.

## Serialization and downstream consumers

CHAT serialization writes the model's representation. Semantic equivalence,
byte-exact source preservation and validity are separate properties; state which
one a test establishes. Derived word spelling is not an original source slice.

Downstream alignment and transformation code should consume typed projections
and explicit admission results. Parse-health metadata helps identify recovery;
it is not a replacement for whole-document validation or an acoustic-quality
judgment. The [JSON contract](../chatter/integrating/json-output.md) likewise
distinguishes computed display fields, model structure and schema checks.

String representation is an implementation choice, not a universal storage
restriction or allocation guarantee. Consult the owning types and
[memory architecture](memory-and-ownership.md) instead of copying layouts into integrations.

## Review and verification

For a boundary change, inspect its producers, consumers and failure paths
together. Keep constructor, wire-format and policy controls even when typestate
prevents an invalid transition later in the pipeline. Exercise the actual public
API and observe diagnostics, model content and output where each matters.

Use the [spec workflow](../contributing/spec-workflow.md) and
[development checks](../contributing/dev-checks.md) for current verification.
A signature-name scan cannot prove honest recovery, complete validation or
backend conformance. Compatibility promises for 1.0 remain a separate
[readiness requirement](../contributing/one-zero-readiness.md).
