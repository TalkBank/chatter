# Library Usage

**Status:** Current
**Last updated:** {{git-dates:page}}

The TalkBank Rust crates can be used as dependencies in your own Rust
projects for parsing, validating, and manipulating CHAT files. This page
shows the most common entry points; the API reference on docs.rs (once
published) is the authoritative source. Until then, treat the rustdoc
comments inside each crate's `src/lib.rs` as the source of truth.

The Rust recipes below are included directly from
`crates/talkbank-transform/tests/integration/book_library_usage_examples.rs`.
Cargo compiles and runs that source; the book renders it without a second
manually maintained copy. The surrounding explanations still require review
when APIs or policies change.

**Important:** some legacy tree-sitter fragment helpers are synthetic
rather than semantically honest. They can inject fragment input into
boilerplate CHAT text and parse the resulting synthetic file. Prefer
full-file parsing for real tree-sitter use, and do not treat legacy
fragment helpers as the long-term fragment API. For direct-parser
fragment semantics, use direct-parser-native tests instead of treating
synthetic wrappers as the oracle.

## Adding Dependencies

The TalkBank library crates are source-available from this repository. They are
not yet published on crates.io, so depend on them from the public repo via git
(pinned to a release tag), or via local path dependencies from a
`TalkBank/chatter` checkout for local development:

```toml
[dependencies]
talkbank-model = { path = "../chatter/crates/talkbank-model" }
talkbank-transform = { path = "../chatter/crates/talkbank-transform" }
talkbank-parser = { path = "../chatter/crates/talkbank-parser" }
```

Registry publication is a [separate acceptance workflow](../../contributing/crates-io-publication.md).
Do not substitute an unrelated package with a similar name. Record the Chatter
commit or release tag used by your application, and use one coherent revision
for these coupled crates.

## Run the library recipes

From the directory containing your Chatter checkout, create a small library
project alongside it:

```sh
cargo new --lib chat-example
```

Add the three path dependencies above to its `Cargo.toml`. Paths are relative
to that manifest, not the shell's current directory. Replace the generated
`src/lib.rs` contents with the shared imports and sample inputs below, followed
by the recipe functions you want to try. These are `#[test]` functions rather
than `main`; `cargo run` is not their entrypoint.

```rust,ignore
{{#include ../../../../crates/talkbank-transform/tests/integration/book_library_usage_examples.rs:book_imports}}

{{#include ../../../../crates/talkbank-transform/tests/integration/book_library_usage_examples.rs:book_samples}}
```

From `chat-example`, run:

```sh
cargo test -- --nocapture
```

The first recipe below prints `Speaker: CHI` and passes. Copying all six recipes
gives six passing tests; copying fewer can leave unused-import/sample warnings.
Use these synthetic inputs before trying research data. For a real file, read
its UTF-8 text with `std::fs::read_to_string(path)?` in a result-returning
function and pass that string to the same API. Do not overwrite the input as
part of learning the serialization calls.

Maintainers can select the actual book examples in the existing integration
target with `cargo test -p talkbank-transform --test integration book_library_usage_examples`.
That checks the included recipes, not external registry installation or every
supported platform.

### Optional library features

`talkbank-model` has no default features. Its `async` feature adds Tokio-backed
validation and `AsyncChannelErrorSink`; `channels` independently adds the
crossbeam-backed `ChannelErrorSink`. Neither feature is required for ordinary
parsing, model validation or checked diagnostic presentation.

The optional Tokio dependency also exposes a `tokio` feature. Enabling it alone
does **not** enable the `async` API; `async` enables `tokio`, not the reverse.
Consequently the effective model combinations are none, `channels`, `tokio`,
`tokio` + `channels`, `async`, and `async` + `channels`. Each can be combined
with the transform runner enabled or disabled. An empty `default` marker does
not enable optional model APIs. Tokio appearing among test dependencies is not
evidence that the model's async API is enabled.

`talkbank-transform` enables `validation-runner` by default. To use its core
library without the batch runner and result cache, opt out explicitly:

```toml
talkbank-transform = { path = "../chatter/crates/talkbank-transform", default-features = false }
```

Parsing, synchronous validation, JSON/CHAT conversion, transforms and diagnostic
rendering remain available. The batch runner, its cache and worker-pool exports
do not. Cargo features are additive across a dependency graph: another dependency
can enable a feature even if this declaration disables defaults.

For an existing model in a Tokio application, `validate_async` consumes the
`ChatFile` and returns `ValidChatFile` only after complete validation. A validation
refusal and a failed blocking task are separate `AsyncValidationError` variants.
`validate_with_rules_async` instead reports selected-rule findings to the supplied
sink; its `Ok(())` means task completion, not document validity. The async rustdoc
examples compile independently with the `async` feature.

## Explicit English number generation

`talkbank_transform::num_words::expand_number` spells supported numeric input
for callers constructing transcripts. It is not a CHAT parser, validator, or
automatic repair of existing speech. The caller remains responsible for the
intended pronunciation; unsupported input is preserved exactly rather than
partially rewritten or assigned a guessed pronunciation.

English ordinal composition supports 0-9999 and retains British-style
conjunctions without prose commas. Larger suffix-bearing ordinals are preserved,
including their original suffix: `10001st` must not become `10001th`.

English decade expansion requires a multiple of ten in one of these domains:

| Input domain | Examples | Generated words |
| --- | --- | --- |
| Shorthand 0-90 | `0s`, `80s`, `90s` | `zeros`, `eighties`, `nineties` |
| Full-year 1100-2990 | `1100s`, `1950s`, `2990s` | `eleven hundreds`, `nineteen fifties`, `twenty-nine nineties` |

Other suffix forms, such as `21s`, `100s`, `2001s`, and `3000s`, remain
unchanged. This deliberately avoids inventing phrases such as “twenty-ones”
or “two thousand and ones”. Neither support nor preservation certifies CHAT
validity: English digit-bearing words still require an authored transcription.
Initial zero has omission semantics in CHAT, so the raw-string `0s` and
leading-zero API cases are not interchangeable with parsed CHAT words.

These are bounded generation policies, not claims about every possible English
number expression or the speaker's intent. The [testing contract](../../contributing/testing.md)
pairs authored written reference controls with invalid numeric specimens and
keeps raw-string boundary tests separate.

## Explicit Dutch ordinal generation

For ISO code `nld` (case-insensitive), the same `expand_number` API accepts
whole-token ASCII digits followed by lowercase `e`, `de`, `ste` or `ᵉ`, for
ranks 1–1000. Examples: `29e` → `negenentwintigste`, `101ste` →
`honderdeerste`, `1000ᵉ` → `duizendste`. A private admitted rank is required
before composition; cardinal words come from the existing Dutch lexicon.
Hundreds join directly to the final ordinal component. This deterministic
choice does not reject other correctly written variants with `en`.

Suffixes are compatibility inputs, not suffix-agreement validation:
`1de` and `2ste` normalize to `eerste` and `tweede`. Leading zeros are
interpreted numerically (`00029e` → rank 29); superscript `ᵉ` is accepted for
generator/ASR compatibility, not recommended as canonical numeric notation.
The [Dutch notation guidance](https://taaladvies.net/1o-2o-3o-of-1e-2e-3e-of-1ste-2de-3de/)
prefers baseline suffix letters and rejects degree signs. Compound-word
variants are documented by [Onze Taal](https://onzetaal.nl/taalloket/honderdeneende-honderdeneerste).

Zero, ranks above 1000, overflow, uppercase suffixes, non-ASCII digits,
embedded letters, decimals, negatives and ordinal-hyphen compounds remain
unchanged. The API does not strip punctuation: `29e.` stays `29e.`; callers
with tokenization may separately expand its `29e` token. Already-written
words and Dutch cardinal/currency rules are unchanged. Neither compatibility
normalization nor preservation establishes a speaker's intended pronunciation
or certifies CHAT validity.

## Parsing and Validating a CHAT File

The convenience entry point is `parse_and_validate` from `talkbank-transform`.
It returns a `ChatFile` after the checks selected by `ParseValidateOptions`,
or a `PipelineError`. The example explicitly requests structural validation;
this is not a claim that every optional policy or tier-alignment check ran.

```rust,ignore
{{#include ../../../../crates/talkbank-transform/tests/integration/book_library_usage_examples.rs:book_parse}}
```

`parse_and_validate` returns a mutable `ChatFile` and may skip validation
according to its options. Use `parse_validated_with_parser` for an immutable
`ValidChatFile` whose policy cannot skip validation.
For an existing model, `talkbank_model::validate_chat_file_with_options` runs
only the selected validation phase. It does not parse or certify recovery-free
input: `Ok(())` can mean validation was skipped, and any emitted diagnostic
(including a warning) produces `Err(Vec<ParseError>)`. It has no filename, so
filename-dependent checks require a named validation entry point instead.
When original bytes must be returned unchanged, use `parse_source_with_parser`.
Its opaque `ParsedSourceChat` permits read-only inspection before `admit`
validates that same parse with complete rules and tier alignment; this
unchanged-output admission has no caller-selected policy. Success returns
`AdmittedSourceChat`, coupling
original bytes with their immutable model proof; `into_owned` keeps the binding
across asynchronous storage. No constructor accepts separate text and model
evidence. `into_product` relinquishes admission authority for recovery work,
and `into_valid_file` relinquishes the original-byte binding before editing.
Removing and regenerating tiers requires the separate replacement admission
below; an unchanged source cannot claim a removal exemption.
`chat_file.utterances()` returns an iterator over `&Utterance` derived
from the file's `lines` (utterances are interleaved with headers and
comments in source order).

For batch workflows where parser construction overhead matters, reuse a
single `TreeSitterParser` and call `parse_and_validate_with_parser`:

```rust,ignore
{{#include ../../../../crates/talkbank-transform/tests/integration/book_library_usage_examples.rs:book_reuse_parser}}
```

`ParseValidateOptions` also exposes `with_alignment()` (implies
`with_validation()`, additionally validates cross-tier alignment for
`%mor`, `%gra`, `%pho`, `%wor`), `with_level(CheckLevel)` to choose the
level directly (`CheckLevel::ParseOnly` or
`CheckLevel::Validate(AlignmentValidation::Structure | IncludeTierAlignment)`),
and
`with_strict_linkers()` (enables the
opt-in cross-utterance quotation and completion checks; `chatter
validate --list-checks` marks each code it turns on as `[Opt-in]`).

## Admission for dependent-tier regeneration

`TreeSitterParser::admit_replacing_tiers` is a separate input boundary for a
transform that will actually remove and regenerate dependent tiers.
`ReplacementTiers::Morphosyntax` removes `%mor` and `%gra` together;
`ReplacementTiers::WordTiming` removes `%wor` while retaining morphology.

For header-dependent policies, `admit_planned_tiers` supplies immutable references
to all typed headers to a one-shot callback. Returning `None` preserves every
tier and requires whole-document validity; returning a replacement selection
physically removes only that domain. Header lowering happens once, before
pending source-bound utterances lower once. The callback sees misplaced headers
too, but cannot excuse header faults. `AdmittedReplacement::selection` records
the actual optional removal selection; an unfinished producer plan is a tool
failure, never CHAT invalidity or validation success.

The parser parses once. Its generated, source-bound traversal selects concrete
tier nodes and omits them before lowering the retained model. The recovery
backstop excludes those exact discarded subtrees, not diagnostic codes or
overlapping byte spans. Headers, main-tier/retrace errors, retained dependent
tiers, unclassified recovery and global forbidden-control checks remain subject
to refusal. The retained model must pass normal validation and tier alignment.

The resulting `AdmittedReplacement` owns both immutable `ValidChatFile` evidence
for the retained document and the original `ParsedSource`. Its removal receipt
records each selected tier's original span and whether it contained syntax
recovery. Absence of syntax recovery does not prove that discarded content was
semantically valid. This API does **not** certify the original bytes as valid
CHAT and never repairs or marks recovered content clean.

A pass-through, keep-tier or incremental preservation path must validate what
it will retain; it cannot use whole-file tier removal to excuse faults in copied
content. After admission, transformations still need completion and output
validation before writing. `into_valid_file` explicitly consumes the
source/removal association; `ValidChatFile::into_unchecked` consumes validity
before editing. No serialized CHAT is reparsed to establish these states.

### Adaptive word-timing regeneration

`admit_word_timing_plan` returns `WordTimingAdmission`, not unconditional
`AdmittedReplacement`. Its disposition is preserved, replaced with a completely
valid retained document, or `Regenerating(AdmittedTimingRegeneration)`.
The last branch is possible only when concrete discarded source-bound `%wor`
tiers contain recorded timing and their removal leaves the linked-media timing
requirement outstanding. The model and original source remain bound to the
removal receipt; no original-byte output proof is issued.

Only the word tiers actually at fault are removed; the others are retained.
Each `RemovedTier` carries a `RemovalCause` naming why: `OwnLowering` (the
tier itself did not lower cleanly), `LocatedValidation` (validation errors
inside its source span), `UnattributedValidation` (errors in no word tier, so
every remaining word tier was removed, sharing those diagnostics) or
`Selected` (a fixed `ReplacementTiers` request). `RemovalCause::diagnostics`
returns the diagnostics that cost the tier, so a consumer can report which
codes cost which tier.

`PendingTimingChatFile` is checked working structure, **not valid CHAT**.
The payload has no `into_valid_file`, accepted-document serialization or
validity constructor. It keeps its document and its `MediaTimingObligation`
together: write the regenerated timing through `document_mut`, then
`discharge` checks the payload's document with the very rule that issued the
obligation (E544), handing that document out once the rule no longer fires
and the payload back while it would. The obligation alone has no discharge,
so the verdict always concerns the document the consumer goes on to admit.
UTR/FA must establish actual timing, and final output must still pass
complete checked construction, including E544. No header is
changed and no emitted diagnostic is filtered. Normal validation still reports
E544; unrelated retained faults, internal failures and preservation/NoAlign
paths still refuse. Missing timing without concrete discarded timing is not
eligible for this adaptive state.

## Working with the Model

`ChatFile` stores participants and language metadata as top-level fields
populated from `@Participants` / `@ID` / `@Languages` headers during
parsing. Utterances live in `lines` and are iterated via
`chat_file.utterances()`.

```rust,ignore
{{#include ../../../../crates/talkbank-transform/tests/integration/book_library_usage_examples.rs:book_model}}
```

Each dependent-tier entry carries its typed value in `entry.tier`; match that
`DependentTier`, not the entry wrapper. Select just `Mor` for this read-only
example. A transform that promises to preserve all tiers must also preserve
the variants it does not change. `MorTier::items()` returns `&[Mor]`; each `Mor`
has a main `MorWord` plus optional post-clitics.

## Serializing to CHAT

Bring the `WriteChat` trait into scope and call `to_chat_string()` for a
fully-rendered CHAT string, or `write_chat(&mut writer)` to stream into
any `std::fmt::Write`.

```rust,ignore
{{#include ../../../../crates/talkbank-transform/tests/integration/book_library_usage_examples.rs:book_chat}}
```

Serialization is not validation and need not preserve the original bytes.
Editing a mutable `ChatFile` can invalidate previous findings. Validate the
edited model before publishing its output; retain the source-admission carrier
described above when the requirement is unchanged original bytes.

## Serializing to JSON

Prefer the schema-validated helpers in `talkbank_transform::json`:
`to_json_pretty_validated` checks the output against the JSON schema and
catches drift between the data model and the schema. Schema conformance and
CHAT semantic validity are different checks: one does not replace the other.
The explicitly unvalidated variants skip the schema check; use that path only
when your application deliberately does not require that guarantee.

```rust,ignore
{{#include ../../../../crates/talkbank-transform/tests/integration/book_library_usage_examples.rs:book_json}}
```

The schema for `ChatFile` lives at `schema/chat-file.schema.json` and is
regenerated from the Rust types via `just schema-gen`. For arbitrary
serde values (not just `ChatFile`), `to_json_unvalidated` /
`to_json_pretty_unvalidated` work the same way without the schema step.

To go straight from CHAT text to JSON, `talkbank_transform::chat_to_json`
parses, performs the checks selected by `ParseValidateOptions` and serializes
in one call. It takes a `JsonLayout` (`Pretty` or `Compact`) rather than a
bare `bool`. It does not implicitly upgrade a parse-only option to validation.

## Custom Error Handling

Lower-level parser entry points stream diagnostics through the
`ErrorSink` trait. Implement it to collect, count, filter, or forward
errors as they arrive, useful when you need finer-grained control than
the `Result<ChatFile, PipelineError>` shape `parse_and_validate` returns.

```rust,ignore
{{#include ../../../../crates/talkbank-transform/tests/integration/book_library_usage_examples.rs:book_sink}}
```

This last recipe checks that a custom sink implements the interface. Its empty
handler intentionally records nothing; use `ErrorCollector` or implement the
actual reporting behavior your application needs. Discarding a diagnostic does
not repair invalid CHAT or turn an internal tool failure into a valid result.

`ErrorSink` is `Send + Sync`, and a blanket `&T: ErrorSink` impl means
borrowed references are sinks too, no `Arc` wrapper required. The
built-in `ErrorCollector` (gathers into a `Vec`), `ParseTracker` (counts
by severity), and `NullErrorSink` (discards) cover most common needs;
implement `ErrorSink` directly for everything else.

## Crate Selection Guide

| Need | Crate |
|------|-------|
| Data model types, error types, `WriteChat`, `ErrorSink` | `talkbank-model` |
| Tree-sitter CHAT parsing (low-level) | `talkbank-parser` |
| Full pipeline (parse + validate + JSON, schema validation) | `talkbank-transform` |

`talkbank-model` is the foundation, every other crate depends on it. If
all you need are the AST types and validation, model alone is enough.
`talkbank-transform` brings parsing and JSON; its default `validation-runner`
feature additionally brings batch validation and caching.

## Batchalign3-Facing Surface

If you are building Batchalign3 or another external consumer, the stable
surface is usually:

| Batchalign3 need | Prefer |
|------------------|--------|
| Canonical full-file parsing | `talkbank-parser` |
| Parse/validate contracts and typed model access | `talkbank-model` |
| Alignment-aware downstream consumers (`align`, `compare`, `benchmark`) | `talkbank-model` alignment helpers plus the model AST |
| Whole-pipeline parse+validate+convert | `talkbank-transform` |

For batch workflows, keep parser instances reusable and keep alignment
logic separate from parse semantics.
