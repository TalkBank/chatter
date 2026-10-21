# CHAT-to-JSON Pipeline

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/docs/CHAT_TO_JSON_PIPELINE.md)

How raw CHAT becomes a parsed, optionally validated and alignment-annotated,
schema-checked JSON representation. The central invariant is preservation:
dependent tiers remain typed tier values. Alignment derives metadata without
consuming tiers or moving their payloads onto main-tier words.

## Pipeline overview

```text
Raw CHAT text
  -> tree-sitter CST
  -> typed ChatFile AST with preserved dependent tiers
  -> selected validation phase (optional)
       alignment-aware: derive AlignmentSet/language metadata, then validate
  -> serde JSON
  -> generated JSON Schema validation
```

`talkbank-transform::parse_and_validate()` orchestrates these stages according
to `ParseValidateOptions`. `chat_to_json()` serializes the resulting `ChatFile`
and validates it against `schema/chat-file.schema.json`.

## Entry point and options

The primary entry points live in:

- `crates/talkbank-transform/src/pipeline/parse.rs`
- `crates/talkbank-transform/src/pipeline/convert.rs`

```rust
pub fn parse_and_validate(
    content: &str,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError>
```

The reusable-parser variant accepts a `TreeSitterParser`, avoiding parser setup
on repeated work. Named/path variants carry source identity for filename-aware
rules. These compatibility APIs return a mutable `ChatFile`, not a retained
validation capability. Their required-validation path uses checked admission
and then explicitly discards that phase proof. Prefer the checked APIs in
[Library Usage](../../book/src/chatter/integrating/library-usage.md#parsing-and-validating-a-chat-file)
when downstream work requires retained validation evidence.

Non-streaming parse-only conversion rejects error-severity parser findings;
skipping model validation does not mean accepting every recovered product.
The streaming parse-only API can return a model while reporting findings to
its sink. Do not substitute it for required validation. Internal producer
failures remain distinct from CHAT invalidity in both paths.

`with_validation()` enables CHAT validation. `with_alignment()` enables the
alignment-aware validation path, which computes derived alignment and language
metadata before validation reports alignment diagnostics. Validation without
alignment remains meaningful; rules such as media-linkage timing inspect typed
main and `%wor` bullets directly and do not depend on optional alignment
metadata.
`with_alignment()` also enables its structural-validation prerequisite.
`with_strict_linkers()` selects rules but does not enable validation by itself.

## Parsing: CST to typed AST

`talkbank-parser::TreeSitterParser` is the parser frontend. Whole-file product
parsing distinguishes `ParseProduct::Built` from `Unbuildable`; a built product
can carry recovery diagnostics and is not source-validity evidence by itself.
Fragment APIs produce `ParseOutcome<T>` so
`Parsed(T)` and `Rejected` cannot be confused with an optional semantic value.

The CHAT-file parser walks the tree-sitter CST and lowers each known node into
its model type:

- headers become `Header` variants;
- main tiers become `MainTier` plus typed `UtteranceContent`;
- dependent tiers become concrete `DependentTier` variants;
- recovery contributes diagnostics and `ParseHealthState` provenance.

Structured tiers such as `%mor`, `%gra`, `%pho`, `%sin`, and `%wor` are retained
directly as `MorTier`, `GraTier`, `PhoTier`, `SinTier`, and `WorTier`. There is
no marker-plus-pending-items phase and alignment does not consume their items.

## Model ownership

### `ChatFile`

`ChatFile` owns ordered lines and derived header summaries. `Line` preserves
the original interleaving of headers and utterances.

### `Utterance`

An utterance owns:

- preceding interstitial headers;
- one typed `MainTier`;
- ordered `DependentTierEntry` values;
- optional derived `AlignmentSet` metadata;
- runtime-only alignment diagnostics and parse health;
- explicit utterance and per-word language-metadata states.

`DependentTierEntry` retains each tier with its parsed separator while
serializing transparently as `DependentTier` on the JSON wire.

### `Word`

A `Word` owns its typed CHAT content, category and annotation markers. It does
not acquire `%mor`, `%pho`, or `%wor` values during alignment. A word's
`inline_bullet` is direct syntax: on a `%wor` word it is the actual parsed or
generated timing observation.

Main-tier lexical identity and dependent-tier evidence therefore cannot be
silently collapsed into one mutable object.

## Validation

`ChatFile::validate()` checks the typed model without computing alignment.
`validate_with_alignment()` first computes the optional metadata needed by
alignment-aware checks, then validates. Both paths share the same parsed tier
values.

File-level media rules distinguish physical timing evidence from
correspondence metadata:

- a main-tier bullet is timing evidence;
- an inline bullet on a `%wor` word is timing evidence;
- an untimed `%wor` tier is not timing evidence merely because its count
  matches the main tier;
- alignment processing is not required to observe a physical bullet.

## Structural alignment metadata

`Utterance::compute_alignments()` lives in
`model/file/utterance/metadata/alignment/compute.rs`. It builds typed unit
inventories and derives an `AlignmentSet` while leaving all tiers intact.

Structural relationships include main-to-`%mor`, main-to-`%pho`,
main-to-`%mod`, main-to-`%sin`, `%mor`-to-`%gra`, and the Phon tier-to-tier
relationships. Count mismatches in these relationships may produce typed
diagnostics. `ParseHealthState` prevents recovery-tainted data from being
reported as a trustworthy alignment result.

## `%wor` timing is a separate typestate pipeline

`%wor` is a timing sidecar, not a structural alignment. `AlignmentSet` retains
legacy count metadata as `WorTimingSidecar::Positional` or `Drifted`; this is
not a validation verdict and does not expose word timing.

A timing consumer uses the stronger API:

```text
bind_wor_timing
  -> Missing | Drifted | CountMatched

corroborate_wor_timing(CountMatched)
  -> Uncorroborated | Corroborated

assess_wor_timing_sequence(Corroborated)
  -> Empty | Rejected | Complete
```

Only `Corroborated` exposes positional slots, with lexical identity borrowed
from the main tier and timing copied from the corresponding `%wor` word bullet.
Only `Complete` exposes a timing hull. Its typed adjacency evidence still
distinguishes gaps, touching intervals, overlap, and backwards starts; none of
these structural states claims acoustic accuracy.

Generation and binding share `WorMainTierProjection`, the single owner of the
versioned `FilteredLexicalV1` membership policy. See
[Wor timing semantics](../../book/src/architecture/wor-timing.md) for the full contract.

## JSON serialization and schema

The model derives `Serialize`, `Deserialize`, and `JsonSchema` where
appropriate. Important wire patterns include:

| Type | Wire form |
|---|---|
| `Line` | internally tagged by `line_type` |
| `DependentTier` | tagged by `type`, payload in `data` |
| `WorTimingSidecar` | tagged by `kind` |
| optional `AlignmentSet` fields | omitted when `None` |
| spans, parse health, diagnostics, caches | skipped |

The dependent-tier array contains the actual structured tiers, not marker
summaries. Optional `AlignmentSet` metadata is adjacent derived evidence. For
`%wor`, serialized `Positional { count }` means only that counts matched under
the legacy convention; consumers needing timing must use the checked binding
API rather than treating JSON count metadata as proof.

`schema/chat-file.schema.json` is generated from the Rust model with schemars.
After a model change, run:

```bash
just schema-gen
```

This explicitly runs the generator and preserves the file when output is
unchanged. Subsequent ordinary tests rebuild against the generated bytes and
check currency without rewriting the schema. `just regen` includes this step.

Production `talkbank-transform` JSON output is checked with the compiled
schema unless the caller explicitly selects the schema-skipping policy/API.
Schema validation checks the wire shape, not CHAT semantic validity, parser
provenance or correspondence with original source bytes. Ordinary `ChatFile`
deserialization does not recreate a checked validation phase or parse health.
See [JSON Schema](../../book/src/chatter/integrating/json-schema.md).

## CHAT roundtrip

CHAT serialization writes the preserved typed values:

1. preceding headers;
2. the main tier;
3. dependent-tier entries in their original order.

No materialization from embedded word state is needed, because alignment did
not consume the tiers. `%wor` serialization writes its word entries and their
inline bullets. A trailing tier-level `%wor` bullet is absent from the grammar,
the Rust type, and the JSON schema.

This writes the typed model, not an archival copy of the original bytes.
Recovery may have discarded source material; normalized formatting and skipped
runtime metadata also preclude treating JSON round trips as source preservation.
Review the relevant semantic and byte-roundtrip contracts separately.

## CLI

The `chatter to-json` and `chatter from-json` commands are implemented in
`crates/chatter/src/commands/json.rs`. `to-json` selects the requested parsing,
validation, alignment, pretty-printing, and schema-validation behavior;
`from-json` deserializes `ChatFile` and uses typed CHAT serialization. It does
not itself run CHAT validation or the JSON Schema validator. Validate the
result separately when required, and use a separate output path to preserve
the original; successful conversion is not an admission receipt.

## Crate responsibilities

| Crate | Responsibility |
|---|---|
| `talkbank-parser` | tree-sitter parsing and CST-to-AST lowering |
| `talkbank-model` | typed AST, validation, alignment metadata, typestate APIs |
| `talkbank-transform` | pipeline orchestration and JSON/schema boundary |
| `chatter` | CLI and user-facing I/O policy |

## Maintainer checks

When changing this pipeline:

1. Change the typed model or transition first.
2. Add a failing behavioral or compiler-boundary test that proves the old
   state was unsafe or the new state is required.
3. Regenerate affected owned artifacts through the root's consolidated
   workflow, including the schema when its model inputs change.
4. Run affected model/transform and wire-boundary tests. Check artifact
   currency when relevant; do not treat it as behavioral acceptance.
5. Inspect generated diffs; regeneration is evidence only after adjudication.
