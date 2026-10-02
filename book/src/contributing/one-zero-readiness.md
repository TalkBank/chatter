# Toward Chatter 1.0

**Status:** Current
**Last updated:** {{git-dates:page}}

This is the current readiness record.
Version 1.0 means a documented compatibility contract and reproducible evidence,
not merely a release-number change. No date is promised here.

## Where the numbers live

`just spec-status` reports the current spec counts (error specs by status,
examples verified, deferred or failing) and the CHECK assessment summary. These
are spec and example counts, not a count of independent validation rules. To
inspect the deferred specs, run
`cargo run --manifest-path spec/Cargo.toml --bin spec_status -- --deferred`:
prioritize rules reachable through supported CHAT input, preserve legal and
invalid examples together, and use the observation snapshot to adjudicate
backend differences.

The [generated CHECK assessment](../architecture/errors-and-validation/check-parity-audit.md)
owns the current scope, adjudication counts, completion status and reopening
criteria. This readiness record does not maintain a second CHECK status.

## Release conditions

- Define the stable CLI, exit-code, JSON/schema and Rust API surfaces. State
  which surfaces remain experimental and test the chosen compatibility contract.
- Adjudicate every deferred spec for the stable surface. Implement missing
  behavior or document why the rule is deprecated, unreachable or deferred;
  do not activate a spec merely by changing its status.
- Ground CHECK obligations against an identified executable and fixture set.
  Keep deliberate divergences justified in the manifest. Every observed
  disagreement needs a spec or validator adjudication before data repair.
- Review typestate transitions at parsing, validation, repair and writing.
  Output-producing APIs must consume the evidence their operation requires;
  a curated label, parsed AST or recovered node is not proof of validity.
- Keep generated artifacts current and reader-facing documentation consistent
  with actual commands, validation behavior and release mechanics.
- Pass the established release gate and platform jobs on the exact release
  commit. Verify packaged artifacts and deployment using the release runbook.

## Evidence boundaries already in place

These are properties of the code that the release conditions build on; each is
a typed or gated boundary rather than a convention.

- **CHECK mapping audit.** It reads the compiled spec registry, renders only
  mapping evidence (unmapped or nonempty curated states) and has a
  report-currency integration gate. Explicit CLAN grounding cannot silently
  succeed without its wrapper.
- **Source-bound diagnostic indexes.** `SourceIndex<'source>` owns the line
  boundaries and immutably borrows their source; its constructor is the only
  way to pair them, so a line map from another source cannot be supplied.
  Indexed callers use `enhance_errors_with_index`. One-off lookups scan the
  source prefix; repeated batches use one explicit index (O(source bytes)
  construction, O(log lines) lookup).
- **Prosodic validation.** `Word` passes through a private `ProsodicWord`
  measurement before its prosodic checks. It borrows the word immutably and
  owns the stress counts and the first and last spoken positions; only its
  constructor can assemble them, so they cannot describe a different word or
  outlive a mutation.
- **JSON conversion.** `JsonSchemaPolicy` selects serialization after the shared
  named CHAT parse, so skipping schema validation cannot discard transcript
  identity or bypass E531.
- **Deserialized model values need validation.** CHAT text cannot construct an
  empty text, phonetic or shortening segment, so E251 is implemented at the
  model validation boundary (serde-constructed values), covered by
  `test_e251_deserialized_word_content`. A wrapper type's name is not evidence
  that deserialized content is valid.
- **Schema.** The generated JSON schema is native Draft 2020-12, with `$ref`
  siblings kept as the dialect allows; consumers must support that dialect. The
  generated `BracketedItem` definition is exercised by
  `generated_ref_siblings_enforce_tag_and_payload`. `just schema-gen` is an
  explicit operation; the normal suite checks currency without writing it.
- **Generated outputs preserve unchanged files.** Generators publish through
  `scripts/generate_if_changed.py` (staging, atomic replace, identical bytes
  keep their mtime, a failed generator leaves the existing file). The spec
  artifact writer deletes only explicitly retired names in shared directories,
  and `GeneratedDir` proves exclusive generated ownership before pruning.
  `just node-types-check` and `just grammar-generate-check` verify these.
  See [Testing](testing.md#regeneration-must-preserve-unchanged-outputs).
- **Publication set.** The publication check derives every non-first-wave
  workspace package from Cargo metadata and requires `publish = false` for it;
  `--metadata-only` runs it without package assembly. It is evidence of
  publication policy and metadata only; full package and registry verification
  remain part of the release review.
- **re2c backend.** It borrows the caller's source, owns only reconstructed
  subtoken text (`Cow<str>`) and has no production `Box::leak`.
  `just verify-vendored-lexer` confirms the committed lexer against the pinned
  generator. `SinTier::from_tokens` returns `Result`; JSON remains an
  unvalidated boundary, so utterance validation descends through `%sin` items
  to report empty tokens.

## Reproducing the CHECK evidence

From the Chatter checkout:

```bash
cargo test -p talkbank-parser-tests --test integration check_mapping_audit:: --offline
cargo test -p talkbank-parser-tests --test integration check_validity_parity:: --offline
CHATTER_CLAN_RUN=/path/to/clan-run.sh \
  cargo test -p talkbank-parser-tests --test integration clan_check_grounding \
  --offline -- --ignored --nocapture
```

The wrapper resolves its default CHECK executable from the CLAN checkout;
`CLAN_BIN_DIR` selects another binary directory. Retain the file-mode PTY
wrapper. Record the CHECK executable, wrapper and parity manifest SHA-256 and
the CLAN checkout HEAD with any result: the executable hash identifies what
actually ran, while the checkout HEAD identifies only the source inspected.
These results cover the committed fixture set, not all possible CHAT input.
