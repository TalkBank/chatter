# Toward Chatter 1.0

**Status:** Current
**Last updated:** 2026-09-05 20:17 EDT

This is the current readiness record; the v0.1.0 checklist is historical.
Version 1.0 means a documented compatibility contract and reproducible evidence,
not merely a release-number change. No date is promised here.

## Baseline, 2026-09-05

`just spec-status` reports 223 error specs: 179 implemented, 37 not implemented,
five unreachable from CHAT and two deprecated. Of 418 examples, 368 satisfy
their claims, 50 are deferred, and none fails. These are spec/example counts,
not a count of independent validation rules.

The CHECK manifest records 131 parity cases, ten divergences and 23 no-obligation
cases. Those are adjudicated declarations; refresh the real CHECK grounding
before treating them as evidence for a particular CLAN build. The mapping
inventory cannot certify runtime parity.

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

## First evidence-boundary correction

The mapping audit formerly inferred full semantic/behavioral parity from code
names and read an obsolete Rust source path. It now reads the compiled spec
registry, renders only mapping evidence, and has a report-currency integration
gate. Explicit CLAN grounding cannot silently succeed without its wrapper.

Next: inspect the 37 unimplemented specs with
`cargo run --manifest-path spec/Cargo.toml --bin spec_status -- --deferred`.
Prioritize rules reachable through supported CHAT input, preserve legal and
invalid examples together, and use the observation snapshot to adjudicate
backend differences.

E212's former legal-only sample is now accompanied by a whole-file violation
and a CA-mode legal control. An explicit category prefix prevents the CA
normalizer from rewriting `0(the)`; the retained standalone shortening reports
E209 and E212. `(the)` and `0the` remain clean controls. This closes E212's
reachability audit without weakening normalization or inventing an error on the
historical legal example. The observation snapshot records exact diagnostics
and byte-exact round trips for all three examples.

## Verification recorded on 2026-09-05

The mapping currency/invalid-reference tests passed (2 tests), as did the
manifest/source agreement and Chatter fixture checks (2 tests). The explicit
real CHECK grounding test passed in 12.59 seconds. These results cover the
committed fixture set, not all possible CHAT input.

Reproduce from the Chatter checkout:

```bash
cargo test -p talkbank-parser-tests --test integration check_mapping_audit:: --offline
cargo test -p talkbank-parser-tests --test integration check_validity_parity:: --offline
CHATTER_CLAN_RUN="$HOME/talkbank/scripts/clan-sources/clan-run.sh" \
  cargo test -p talkbank-parser-tests --test integration clan_check_grounding \
  --offline -- --ignored --nocapture
```

The wrapper resolved its default CHECK executable at
`$HOME/talkbank/OSX-CLAN/src/unix/bin/check`; `CLAN_BIN_DIR` was unset.
Adjust the paths for another machine and retain the file-mode PTY wrapper.
Observed provenance:

| Input | Identity |
|---|---|
| Chatter base, plus this readiness patch | `cb112022daa7ffed9de1f1f7583127b62c2bc55f` |
| CHECK executable SHA-256 | `b8b0f919c4e5388fa32aa4ef8f603840c2c339d80c481eba445c605cc942e8a0` |
| Wrapper SHA-256 | `e259fc030423be1c0939d3d230a93e54525016f5b513af61c166fcd5a998e810` |
| Parity manifest SHA-256 | `58cc045f3fd878b0430eb416f5ccd35d35b39d3b4a1ab71febe64d1c02af4fb3` |
| CLAN checkout HEAD | `e35836cd51f42606fc6d8302ad5acca5a7895cce` |

The CLAN checkout has local modifications to five library data files (DSS,
IPA fixes and KIDEVAL reference values). Its HEAD identifies the source
checkout inspected, not a verified build provenance for the pre-existing
CHECK executable. The executable hash identifies what actually ran.

The required `just regen` cycle produced no additional artifact changes; the
walker provenance change from the TSGU update remains the only generated-code
difference. `just test` passed after repairing the hygiene scanner: it mistook
an unrelated nested `fn report(...)` declaration for a call to a failing helper,
and missed a real call with whitespace before `(`. A failing-then-passing
regression covers both. The accepted compile-only book example stays accepted;
no exception was removed to hide the scanner defect.

Final review: the report reuses the compiled registry and existing mapping
owner; supplementary mappings now use enum variants instead of allocated
strings. Mapping evidence has only unmapped and nonempty curated states. The
library owns parsing and rendering, with a thin binary and an integration
currency test. The hygiene scanner remains a lexical heuristic, not full Rust
name resolution. No validator behavior or corpus transcript was changed.


## First deferred rule adjudicated

E246 is now correctly marked implemented. Its original `:hello` example is
separator spacing and asserts subsumption by E765. A reachable `(he):` example
emits E246 and E209, while `hel:o` and `(he)l:o` remain legal. The validator did
not change: this activates coverage that the stale registry status disabled.
The observation snapshot and generated corpus preserve all four boundaries.


## Prosodic validation evidence

`Word` now passes through a private `ProsodicWord` measurement before its
prosodic checks. The measured state borrows the word immutably and owns stress
counts plus either an absent spoken extent or its first and last positions.
Only its constructor can assemble these facts, so they cannot be supplied for
a different word or remain usable across a word mutation.

Placement detection now uses two linear passes and constant-time neighbor
queries. Previously each marker searched a prefix or suffix again. The cost
of allocating emitted diagnostics is additional; no end-to-end speedup
percentage is claimed. Existing E244-E252 rules and diagnostic order are
preserved. The old validation call failed to compile before being migrated;
33 word-validation tests and strict model Clippy passed. The required
`just regen` and `just test` cycle also passed; the per-example diagnostic
observation snapshot did not change.


## JSON conversion policy boundary

`JsonSchemaPolicy` now selects serialization after the shared named CHAT parse.
Skipping schema validation cannot discard transcript identity or bypass E531.
The single-file CLI regression reproduced the former false success; both schema
policies now reject mismatched media names and accept matching names. Directory
coverage also checks that invalid output is absent while a valid sibling is
written. Directory failures now expose their diagnostics and return exit 1.

### E251 deserialization boundary

E251 is implemented at the model validation boundary, but CHAT text cannot
construct an empty text, phonetic or shortening segment. Its status now records
that distinction. Its historical malformed word sample moved unchanged to E342
as an active missing-element example. Tree-sitter reports E255/E342; re2c
reports E209/E253/E255. The existing recovery discrepancy is now measured and
recorded in the backend divergence baseline; neither backend emits E251.

`test_e251_deserialized_word_content` covers all three serde variants with
empty and non-empty values in the existing model test binary. Constructor
documentation now states that serde values require validation; a wrapper name
alone is not evidence that deserialized content is valid.

Reproduce with `cargo test -p talkbank-model --lib test_e251`, then `just regen`,
`cargo run -p talkbank-parser-tests --bin audit_check_parity`, and `just test`.
The relocated example increases verified examples to 365 and reduces deferred
examples to 51. This does not add a CHAT parity claim or change CLI behavior.

The observation snapshot changes only the sample identity from E251 example 1
to E342 example 3. Its parse code, validation code and round-trip observation
are unchanged; this is an intended ownership correction, not altered parser
behavior.

This change also exposed a missing regeneration step: model documentation is
embedded in the JSON schema, but `just regen` did not refresh that schema.
It now runs the existing, exactly selected schema generator before subsequent
builds embed the result. The full suite still checks currency independently.

Validation: the expanded E251 regression, final `just regen` and `just test`,
book build and documentation-date check passed.

### Schema generation and the inner development loop

A regression reproduced that writing identical schema output changed the
file's modification time. The writer now preserves unchanged files, and its
regression also verifies creation and changed output. Generation is an explicit
ignored operation selected by `just schema-gen`; the normal integration suite
checks currency without writing this compile-time input. The currency failure
also avoids printing the entire schema twice. These changes address unnecessary
invalidation and oversized failure logs; no end-to-end speedup is claimed.

Reproduce with `just schema-gen`, then
`cargo test -p talkbank-transform --test integration`. Compare the schema file's
modification time before and after the latter command; it must be unchanged.

Observed validation: 165 transform integration tests passed, four explicit
operations were ignored, and the schema modification time was unchanged.
The book build and documentation-date check also passed.

### Preserve native Draft 2020-12 schema structure

The schema generator no longer rewrites `$ref` siblings into `allOf`.
[Draft 2020-12 section 8.2.3.1](https://json-schema.org/draft/2020-12/json-schema-core#section-8.2.3.1)
explicitly allows sibling keywords. The removed transform and its five
shape-only tests were based on the opposite claim.

A pre-removal experiment on `a5737d0d` demonstrated a semantic defect in that
transform: for instance `{"$ref":"literal","properties":{"x":1}}`, construct
schema `{"const": instance}` and validate the instance with
`jsonschema::validator_for`. It is valid before calling
`fix_ref_properties_combination` and invalid afterward: the traversal rewrites
literal data as though it were a schema. This is evidence about the general
transform, not a claim that the current CHAT model contains that constant.

The regenerated artifact removes 74 wrappers. Reapplying the old transform to
the new parsed JSON exactly reproduces the previous parsed artifact; no other
schema content changed. The replacement regression
`generated_ref_siblings_enforce_tag_and_payload` exercises the actual generated
`BracketedItem` definition: a valid word passes, an unknown tag fails, and a
numeric `raw_text` fails. Thus both the sibling tag and referenced payload are
checked, instead of asserting one chosen representation.

Reproduce with `just schema-gen` and `just test`. Consumers must support the
schema's declared Draft 2020-12 dialect; a tool that assumes older `$ref`
semantics was already outside that declared contract.

Validation: `just test` passed (2,985 passed, 61 ignored); the book
build and documentation-date check also passed.

### E212 coverage clarification

E212's historical `hello world .` example now declares `legal` rather than
`violates`. Its spec distinguishes isolated-word rejection, CA-dependent
main-tier fragment rejection, and invalid CA-omission model representations.
The code remains deferred: none of those entry points alone proves that a full
CHAT file can reach it after normalization. No coverage gain or parser parity
change is claimed. The next adjudication is full-file reachability, with named
API/model regressions required if it is classified as model/fragment-only.

The form-marker generator and remaining spec instructions now point to
`just schema-gen`; the old advice to run a writing test twice has been removed.

Validation: final regeneration and `just test` passed (2,985 passed, 61
ignored); book and documentation-date checks passed. The observation snapshot
is unchanged.

### Publish generated Rust files only after successful generation

At `3b3393a6`, running `just traversal-gen` left the generated bytes identical
but changed `generated_traversal.rs`'s modification time. Direct shell
redirection also truncated the destination before the generator could succeed.
The node-types, traversal and conformance-inventory recipes now stage stdout
through `scripts/generate_if_changed.py`: failed commands preserve the existing
file (or its absence), identical bytes preserve mtime, and changed output is
replaced atomically in the same directory with existing permissions retained.

Two subprocess regressions cover creation, changed output, identical output,
permissions and partial-output failure. `just node-types-check`, already in the
normal gate, runs them. No Rust test binary was added. The inventory generator's
existing `--stdout` mode is reused rather than changing its public interface.

Reproduce the targeted check with `just node-types-check`. For the real recipe,
compare SHA-256 and nanosecond modification time for `node_types.rs`,
`generated_traversal.rs` and the conformance `inventory.rs` before and after
`just regen`. On unchanged inputs both should remain identical. This does not
claim that all generators preserve timestamps or that compilation is eliminated;
`tree-sitter generate` and other tools still own their own output behavior.

Observed validation: all three real generated files retained their bytes and
nanosecond modification times through `just regen`. `just test` passed
(2,985 passed, 61 ignored), as did the two publication regressions, node-type
currency, CI/gate consistency, book build and documentation-date checks.

### Stage tree-sitter outputs and keep the grammar gate read-only

On `cedb52e9`, direct `tree-sitter generate` preserved the bytes of `parser.c`,
`grammar.json` and `node-types.json` but changed all three modification times.
The normal gate also ran that writing command. It now generates into a staging
directory with the CLI's `--output` option. Publication reuses the existing
changed-file helper; `--check` compares without publishing. All emitted files,
including the three C headers, are checked instead of only the three core files.

The real generation and check commands both preserved bytes and nanosecond
modification times for all six artifacts. A regression supplies stale staged
output and confirms check mode leaves existing files alone; another failure
path confirms partial output is not published when the generator exits nonzero.
The node-type check runs these alongside the shared publication tests.

Reproduce with `just node-types-check`, `just grammar-generate-check` and
`just grammar-test`. No generated content or parser semantics changed. This
reduces unnecessary build invalidation; no end-to-end timing improvement is
claimed.

Real CLAN CHECK grounding was also refreshed at `cedb52e9`: it passed in
12.45 seconds using the same executable, wrapper and manifest hashes listed
above. This grounds the curated fixtures, not an assertion of universal parity.

Validation passed: three publication regressions, all 231 grammar corpus
parses and query checks, CI/gate consistency, book build and documentation
dates. The generated artifact diff is empty.

### Preserve shared-directory spec artifacts

The spec artifact writer deleted every current named output before writing it,
including generated model code and Rust test bodies. `Ownership::NamedFiles`
now deletes only its explicitly retired names, then compares current bytes
before writing. Read failures are reported rather than treated as missing
files. The existing ownership enum still controls which files may be deleted;
whole-directory artifacts keep their existing clearing policy.

The regression first reproduced an unchanged file's timestamp changing, then
also caught the progress count claiming a write when none occurred. It now
checks creation, unchanged output, changed output, retired-file removal and
preservation of another producer's file. The returned write count reflects
actual writes. Reproduce with the generators library tests and `just regen`.

Observed validation: `just regen` preserved both bytes and nanosecond
modification times for all 1,149 tracked Rust files. The generator library's
48 tests and strict Clippy passed. Final `just test` passed (2,985 passed,
61 ignored), as did book and documentation-date checks. Generated content and
spec coverage counts are unchanged.

### Prune obsolete generated files without rebuilding directory contents

`GeneratedDir` now proves exclusive generated ownership before pruning. It
reuses `WritableDir`'s rejection of human/ambiguous ownership, requires an
existing directory's generated marker, and scans for protected or linked
entries before deleting anything. The former `clear_owned` API is removed.
Only obsolete files are removed; unchanged fixtures and documents retain their
modification times. Empty directories need not be deleted because they are not
tracked artifacts. Shared-directory ownership remains a separate policy.

Regressions cover unchanged fixtures, stale-file removal, conflicting ownership,
nested human markers and symlinked entries. A linked ownership marker is
rejected before its target can be overwritten. They also verify that a failed scan
preserves stale files rather than partially pruning the directory. No corpus
content or validity expectation is changed by this filesystem policy.

Validation passed: 51 generator library tests, strict generator Clippy,
regeneration and the main workspace suite (2,985 passed, 61 ignored). Only four
generated ownership-marker descriptions changed. A subsequent no-op regeneration
preserved bytes and modification times of all 3,815 tracked files; regeneration
took 8.177 seconds and the following workspace tests took 10.625 seconds, with
no compilation in either command. The bounded nextest trial did not improve
the warm generator suite. Commands and measurement limits are in
[Testing](testing.md#regeneration-must-preserve-unchanged-outputs).

### Make the foundation publication set exhaustive

The release audit confirmed `talkbank-llm` was publishable by default despite
being absent from the first-wave set. The check now derives every other
workspace package from Cargo metadata and requires `publish = false`, removing
the incomplete second list. The new check first rejected `talkbank-llm`; its
manifest now explicitly holds publication back. All seven foundation packages
and eight held-back packages pass metadata validation. Missing first-wave
packages report a diagnostic instead of later indexing a missing entry.

`--metadata-only` makes that check available without package assembly or a
registry dry run. This is evidence of publication policy and metadata only;
no package has been published, and full package/registry verification remains
part of the release review.

### Bind diagnostic indexes to their source

Two source-identity regressions were reproduced before the final refactor:
editing a string in place without changing its address or length reused stale
line positions, and supplying another source's line map to diagnostic enrichment
panicked while slicing a multibyte character. A buffer address is not evidence
of source identity, and two independent arguments cannot prove they belong
together.

`SourceIndex<'source>` now owns the line boundaries and immutably borrows their
source. Its constructor is the only way to pair them. The former public
`enhance_errors_with_line_map` bypass is removed; indexed callers use
`enhance_errors_with_index`, while `enhance_errors_with_source` retains its
signature. A compile-fail example proves the source cannot be edited while its
index is still used. This is a breaking library API change for the next minor
release, not a CHAT format change.

The hidden thread-local cache is removed. One-off coordinate lookup scans the
source prefix without allocation or retained source memory. Repeated batches
use one explicit index: O(source bytes) construction and O(log lines) lookup.
No end-to-end speedup is claimed. These types address diagnostic source binding;
they do not yet close every `Span::DUMMY` or model-provenance boundary.

Reproduce with `cargo test -p talkbank-model --lib --locked` and
`cargo test -p talkbank-model --doc SourceIndex --locked`. The model suite passed
with 647 tests passed and eight ignored; the source-index examples cover both
usable public API and compile-time rejection of mutation.

## Borrowed parser storage and gesture admission, 2026-09-05

The re2c parser now separates source lifetime from temporary token-storage
lifetime. It borrows the caller's source, drops token and recovery buffers when
parsing returns, and owns reconstructed subtoken text through `Cow<str>`.
The generated lexer uses checked EOF reads without a required NUL-padded copy.
This removes all production `Box::leak` calls in the backend and restores
caller-source spans for word fragments. `just verify-vendored-lexer` confirms
the committed output against the pinned generator.

Gesture token construction now has one checked API. `SinToken::new_unchecked`
is removed, `SinTier::from_tokens` returns `Result`, and re2c grammar admission
produces typed `SinToken` values. JSON remains an editable, unvalidated boundary:
utterance validation now descends through `%sin` items and groups to report
empty tokens using the tier's span. The regression first observed zero errors
for two empty tokens, then both errors after the validation chain was wired.

Reproduce with `cargo test -p talkbank-model -p talkbank-parser
-p talkbank-parser-re2c --locked` and `just verify-vendored-lexer`. The targeted
three-crate run passed all tests and doctests (45 ignored); this is not a
current whole-workspace release gate. Reconstructed word spans and other
unchecked constructors remain separate review work.

## E212 reachability verification, 2026-09-05

`just regen`, `just test`, and `just spec-status` passed after adding the E212
boundaries and activating its existing implementation. The workspace run
passed 2,990 tests with 61 ignored. The schema diff only refreshes the gesture
constructor documentation from the preceding API change; no wire shape changed.
