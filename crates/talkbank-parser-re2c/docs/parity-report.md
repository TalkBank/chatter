# Re2c parser parity

**Status:** Current
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-parser-re2c/docs/parity-report.md)

The [Backend compatibility mandate](../../../book/src/architecture/parser-backends.md#backend-compatibility-mandate)
is the single policy authority for this report. Diagnostic-set agreement is
a measurement, not the definition of correctness or a demand to emulate
tree-sitter recovery.

The re2c backend is important for 1.0. Every input that produces a tree-sitter
error must produce some error through the corresponding supported re2c API and
validation policy. Exact diagnostics and recovered models may differ. Tree-sitter
remains the default production validator; neither backend defines CHAT policy
by its observed behavior. Known silent invalid input is a defect, not an
experimental exemption or an acceptable baseline entry.

## Reproduce the invalid-example comparison

From the repository root:

```bash
cargo test -p talkbank-parser-re2c --test integration \
  error_parity::backends_report_every_tree_sitter_error -- --exact --nocapture
```

The harness reads declared examples from `spec/errors/`, parses and validates
with both native backends. It includes legal claims and not-implemented specs:
an absence claim about one code does not assert that the whole input is valid.
Parsing and shared-validation findings retain their actual severity and source
location separately. The comparison asks:

- Whenever tree-sitter emits an error, does re2c emit some error?
- How many diagnostic-code sets agree? This is informational, not acceptance.

Every example is measured as authored and with CRLF and CR transport newlines.
The variants retain the declared source filename and validation-rule selection;
only newline bytes change. All variants contribute their failure inventories,
so a passing transport cannot hide another transport's missed detection.

The separate `equivalence_tests::equivalence_reference_corpus` control exercises
every valid reference file under the same three transports. Both backends must
accept without parse errors and agree semantically for each variant. This is
cross-backend agreement per transport, not a comparison of each backend's model
against its own authored-input model.

Agreement alone cannot establish correctness: both backends could miss the
same rule. Different code sets also require adjudication; the larger set is
not automatically the better answer.

Silence and warning-only output fail required detection. Internal failures
refuse measurement rather than counting as input rejection. The former
diagnostic-set baseline no longer controls acceptance; historical observations
remain below and in Git history. No baseline can exempt a missed error.

A reviewed architectural difference may remain deliberately. An exact-code
expectation failure does not by itself prove a missed validity rule. Classify
the semantic outcome and user impact under the compatibility mandate before
changing an implementation. Passing this finite file-level check does not prove
specification completeness, fragment APIs or each fault within a multi-fault
input. Retain valid controls and isolated invalid variants so an unrelated
error cannot mask missing detection.

## Measured snapshot

### Full-file transport comparison

The expanded comparison measures 1,343 examples from 300 specifications under
each of the three transports. Each has 881 required detections satisfied,
462 inputs without a tree-sitter error, no missed detection and no internal
failure. All 482 legal-claim examples and 19 not-implemented specifications
remain included. Diagnostic code sets differ in 283 authored/CRLF examples
and 277 CR examples; those differences do not change the detection verdict.

The valid-reference control passes 483 observations (161 files × three
transports). The nineteen-test error-parity selection and newline-conversion
control also pass. These finite checks do not establish complete CHAT validity,
fragment coverage, cross-transport model invariance or release acceptance.

### Native comparison admission

The standalone MOR, GRA and word comparisons now fail on a mismatch or
rejection instead of merely printing a failure count. Each parser call owns
its diagnostic sink. Only a returned model without error-severity findings
can enter semantic comparison; an internal tool failure also refuses admission
even if presented as a warning. Ordinary warning identity is not required.
The batch reports every failed observation and refuses an empty population.

Full-file reference comparisons now use this same checked admission owner.
Their former severity-only filter admitted an internal failure reported as a
warning; a planted finding on a real parsed reference reproduced that gap.
The retained regression refuses that finding independently on either backend.
The complete fourteen-test equivalence selection passes, including all 483
reference transport observations and the reference fragment population below.

A planted word substitution demonstrated the former false pass: five of six
comparisons disagreed, yet the test returned success. The same substitution
fails under the repaired verdict. Instrument controls retain mismatches,
rejection, error-bearing models and internal failures in either backend,
alongside accepted equality, ordinary warnings and empty-batch refusal.

The existing 200 MOR, 200 GRA and six word controls remain. An additional
reference-source comparison exercises 3,060 main-tier lexical words, 233 MOR
bodies and 224 GRA bodies through their fragment APIs, using original source
slices and document origins. Word calls retain the file's CA interpretation.
These are finite semantic comparisons, not full contextual validation,
source-span equality, recovery-model identity or canonical coverage evidence.

```bash
cargo test -p talkbank-parser-re2c --test integration equivalence_tests::
```

The marker-chain comparison uses the authored retrace reference for valid
annotation order. Independent checks locate annotations on the retraced word,
group or replacement when they precede the marker, and on the retrace when
they follow it. Both backends must retain those meanings and accept the valid
reference through parsing and validation. Invalid adjacent markers and
wordless retraces instead belong to E377/E378's authored examples and the
one-way detection test. They do not require equal recovered models. The
legacy grouped reverse-order and event-context cases remain represented in
those specs rather than disappearing with the invalid-model comparison.

### October 11 contract transition

Before changing the comparison, the existing diagnostic-set ratchet measured
816 positive/subsumption examples across 294 specs: 563 equal sets and 253
differences. Re2c was silent on 56 invalid examples; a malformed media header
additionally produced only W109 where tree-sitter also reported E316. The old
comparison omitted 459 legal claims and 19 not-implemented specs. These are
demonstrated detection gaps, not a demand to reproduce all 253 diagnostic sets.
The replacement initially measured all 1,300 examples across the same 294
specifications, including 469 legal claims and all 19 not-implemented spec
files. Tree-sitter produced an error on 850 examples; re2c initially missed
61 of those. These are a complete inventory for this population, not 61
independent proofs of specification completeness.

Native repairs admit form suffixes through the shared registry before word
lowering, refuse incomplete word carriers and empty angle groups, retain
orphan-annotation and separator errors, validate structured header lists,
reject unsupported bullet suffixes and phonology colons, preserve native
text-tier lexer errors, and require morphology terminators. Numeric `@Page`
headers remain supported; arbitrary free-text payloads do not become numeric
page carriers. Locations for newly reported regions come from the same lexer
run that owns their source bytes.

The all-example comparison passes after these repairs: all 850 required
detections are satisfied, with zero missing errors or internal tool failures.
The 450 examples without a tree-sitter error remain included; the implication
does not impose reverse-direction error equality. Actual code sets remain
informational. Neither this result nor the authored claims certify all CHAT.

### Separate fragment assessment

```bash
cargo test -p talkbank-parser-re2c --test integration \
  error_parity::fragments::isolated_faults_require_errors_at_supported_fragment_boundaries \
  -- --exact --nocapture
```

The isolated deck has 24 control/mutation pairs over 15 native API routes,
each tested at origins zero and 25 (after a Unicode source prefix): 48
independently admitted, diagnostic-free controls and 44 required tree-sitter
parse-error detections. Four header mutations produce no tree-sitter error
at the fragment parsing API, unlike their whole-file observation; they have
no antecedent under this parse-only policy. This distinction is retained,
not inferred away using whole-file diagnostics.

The assessment exposed silent trait-adapter rejection, dropped phonology
failures, prefix-only GRA relation acceptance, the wrong comment lexer mode,
and a generic dependent-tier adapter that could not admit its valid control.
Adapters now report native rejection through admitted caller coordinates.
Comment fragments use the picture-aware mode; dependent-tier fragments reuse
the existing native prefix dispatcher without a fabricated main tier or file.
Text-tier bullet admission also rejects numeric overflow before lowering.

The deck passes, with error locations contained in the original caller
region. It measures parsing only, not contextual shared validation, every
supported API, every possible fault, rejection/model identity, or fault
multiplicity. Broader fragment coverage and specification completeness remain
separate 1.0 obligations.

### Remaining fragment API surface

The continuation deck measures the nine remaining base APIs: ID headers,
single participant entries, and `%cod`, `%exp`, `%add`, `%gpx`, `%int`, `%spa`
and `%sit` bodies. It also observes utterance cardinality separately. Each
route is invoked both implicitly and through its explicit empty-option-context
overload at origins zero and 25. Valid controls must return models with equal
meaning and no diagnostics; recovered fault-model equality is not required.

The initial inventory exposed 40 missed-error observations. Native ID admission
now consumes a complete header through the existing header owner instead of
folding its prefix into the body lexer's language field. Single-participant
admission refuses multiple entries. Direct participant and text boundaries use
the existing shared control-character rule before lexing, so the legacy NUL
sentinel cannot silently hide caller bytes. The body-only ID helper remains a
distinct low-level API.

The repaired deck passes 40 clean semantic controls and all 72 required
detections; eight mutations have no tree-sitter parse-error antecedent and
remain included without inventing a new rejection requirement. With the
earlier deck, every base fragment method has a finite probe. This is not proof
of every fault, context-sensitive policy, contextual validation or specification
completeness.

```bash
cargo test -p talkbank-parser-re2c --test integration --features trait_tests \
  error_parity::remaining_fragments:: -- --nocapture
```

Enabling the existing `trait_tests` feature exposed obsolete constructor and
private-field accesses in dormant fixtures. They now use checked MOR conversion,
source-owned GRA lowering and read-only accessors. Its 24 selected trait/conversion
controls pass, including semantic agreement for a contiguous MOR word and
error detection for a newline-bearing mutation in both backends. The earlier
isolated fragment deck also passes with the feature enabled. Broader feature
and platform completeness remains separate.

### Context and original-source boundary matrix

The next finite matrix covers all 24 base fragment routes. Each is called
implicitly and with the four declared option sets (empty, `NoAlign`, `CA`,
and their combination), at origins zero and 25. Five original-input mutations
cover emptiness, a structural fault, NUL followed by additional bytes, a
forbidden control character and trailing source. The matrix also separates
CA omission reinterpretation from context-independent notation, ordinary
shortenings and explicitly omitted words; it includes folded headers and
distinct LF/CRLF sources. Unknown option policies and other file context
remain separate obligations.

The first complete inventory exposed 250 missed detections, plus contextual
meaning/refusal and coordinate defects. Its 621 failed observations include
repeated option/origin combinations, not 621 independent defects. Native
context overloads now use the shared CA normalization owner. A source-bound
word-shape admission is shared by normalization and context refusal, including
non-lexical markers; unrelated word categories do not acquire a CA requirement.
An additional interpretation probe exposed eight native word-context mismatches
that ordinary-word controls could not detect. The canonical trait already
forwarded its context; re2c now does so through the existing word operation.
Native token-fragment boundaries apply the existing character rule before
lexing can treat NUL as end-of-input. Missing participant roles, empty
structured tiers and an absent utterance are no longer silent. Direct text
grammar refusal reaches the sink instead of only returning an empty recovery
payload. This does not impose rejection rules on no-antecedent mutations.

Canonical adapters now project wrapper-owned enclosing tier metadata to the
caller body before extraction. Supplied LF/CRLF stays in the admitted input;
only a missing wrapper terminator is synthesized. The MOR-word companion
terminator has its own source projection, so synthetic suffix bytes cannot escape into diagnostics
or their context snapshots. An empty caller has a known diagnostic boundary
at its admitted origin; legacy unlocated model metadata remains distinct.
This is not a model-wide located/unlocated redesign or proof that every model
span is known.

The pre-relocation consolidated feature-enabled run passes 20 primary public-API
controls, 28 native tests (the matrix, a separate word-context probe, both
earlier fragment decks and 24 selected trait/conversion controls), and nine
affected corpus contracts. The matrix satisfies all 1,050 required detections,
364 clean semantic controls and 36 omission refusal observations without CA.
The independent word probe passes 40 semantic/coordinate controls, with
explicit category/content witnesses for the declared CA interpretation.
Standalone word parsing retains its own policy; it does not inherit the
main-tier refusal for omitted enclosing context.
Three primary context/source regressions now have one owner in
`talkbank-parser-tests` under `fragment_corpus::contextual_fragment_contracts`.
They use the public `ChatParser` methods and are selected by the existing
canonical fragment family. NUL/control-character fault injection remains in
the separate primary/native boundary decks; relocation is not itself a
coverage measurement or a new backend-completeness claim.
The matrix's 150 mutations without a tree-sitter parse-error antecedent remain measured,
not classified as valid. Model rebasing is compared within each backend;
diagnostic locations, labels and independently owned context snapshots are
checked for original-source bounds and exact origin translation. Invalid
cross-backend model equality is not an acceptance condition.

```bash
cargo test -p talkbank-parser -p talkbank-parser-re2c -p talkbank-parser-tests \
  --test integration \
  --features talkbank-parser-re2c/trait_tests --locked -- \
  context_public_api:: error_parity::contextual_fragments:: \
  error_parity::fragments::isolated_faults_require_errors_at_supported_fragment_boundaries \
  error_parity::remaining_fragments:: \
  parser_fixtures::trait_ parser_fixtures::convert_ \
  fragment_corpus::contextual_fragment_contracts:: \
  fragment_corpus::invalid_timing_states_from_specs_rebase_to_document_coordinates \
  fragment_corpus::reference_dependent_tier_fragments_preserve_source_models \
  fragment_corpus::spec_documents_preserve_utterance_adapter_recovery_under_rebasing \
  fragment_corpus::dependent_composition_contracts:: --nocapture
```

The first canonical run retained four failures. Original line endings were
being omitted from projected container ranges; two expectations also confused
known empty diagnostic boundaries or synthetic line endings with caller
locations. Caller bytes now remain intact and the expectations use the
actually supplied range. Existing recovery, validation-only retention,
inverse-rebasing and ordinary/contextual word-corpus controls remain in place.

Earlier whole-file/spec/reference acceptance is retained, not represented as
a fresh whole-file run. This finite matrix proves neither all possible faults
nor contextual shared validation, source/feature/platform completeness or 1.0
readiness.

### Valid-reference contract

The reference comparison requires both backends to return a model without
error-severity parse diagnostics, then compares the intended shared-model
semantics. It includes every readable reference fixture, with no diagnostic
exemptions. Warning identity and equality of recovered invalid-input models
are not requirements. This is parse acceptance and semantic comparison, not
full validation, roundtrip, source-span or specification-completeness proof.

The October 11 native-repair check passes all 160 current reference fixtures
under this contract. The numeric `@Page` carrier remains accepted. The focused
native fixture, model/wire, rejected-morphology and fragment-coordinate
controls also pass; these are bounded checks, not a full release receipt.

```bash
cargo test -p talkbank-parser-re2c --test integration \
  equivalence_tests::equivalence_reference_corpus -- --exact --nocapture
```

Participant-list parsing treats lexer-owned continuation tokens as separators,
preserving the pending entry/comma state. The existing participant fragment
contract checks the multiline reference with LF, CRLF and CR separators at a
nonzero origin, alongside its malformed trailing-comma location control.

The October 10 check passes all 147 reference files under this contract. Six
header/recovery/panic controls pass; the invalid-spec comparison remains red on
12 unrecorded differences in word recovery, E375 and W109. Its 689 declared
cases produce 495 equal diagnostic sets and 194 differences; tree-sitter meets
689 expectations, re2c meets 521, misses 128 and is silent on 40. No new baseline
entries were added. Those examples have single-line participant headers and do
not exercise the continuation transition. Reference acceptance does not close
these experimental gaps or certify full backend equivalence.

### Interim 0.27 assessment (2026-09-28)

The comparison measured 669 invalid/subsumption cases across 267 spec files:
487 equal diagnostic sets and 182 differences. The canonical parser met all
669 authored claims; re2c met 511, missed 119 and was silent on 39. Another
374 legal claims are checked separately, and 19 not-implemented cases were
skipped. These dimensions must not be conflated with coverage percentages.

The updated baseline records native recovery-code differences after removal
of CHECK-specific classification, plus existing experimental gaps exposed by
the expanded suffix and morphology controls. It does not endorse silent
acceptance. The default parser remains required for production validation.
Reference comparison also found lost multiword participant names and literal
continuation tokens in gem labels; conversion now preserves all name fields
and projects lexer-owned continuation tokens to logical label separators.

### Interim 0.26 specification expansion (2026-09-24)

The expanded invalid-example comparison measured588 cases across248 spec
files:444 equal diagnostic sets and144 differences. Tree-sitter satisfied all
588 declared expectations; re2c satisfied472, missed86 and was silent on30.
The325 legal-claim examples are checked separately by the fixture runner.

New baseline entries retain the observed diagnostic sets for malformed words,
headers, tier boundaries, scoped annotations, pictures and timing overflow.
The canonical results match the authored specifications; the experimental
backend's generic, missing or extra diagnostics do not supersede those rules.
Space-indented tiers remain a known re2c recovery limitation. This inventory
records incomplete behavior for the interim release, not a parity certification.

### Inline all-zero bullet repair (2026-09-22)

The canonical `E360.md#4` example exposed re2c silently accepting an all-zero
inline bullet on `%com` while tree-sitter emitted E360. Text-tier tokens now pass
a shared admission step before full-file or fragment conversion. It reports
E360 at the original lexer span and omits the rejected bullet without discarding
surrounding content. Fragment diagnostics use the admitted origin-rebasing sink.

Focused re2c verification passed 36 unit and 272 integration tests, with 20
ignored. The existing invalid-spec comparison passed without adding a divergence
entry. A canonical-fixture regression checks E360 locations through both
backends at zero, ordinary and near-maximum fragment offsets. This fixes that
specific silence; it does not establish complete backend parity or supersede
the historical full diagnostic-count measurement below.

### Earlier full diagnostic-count measurement (2026-09-07)

The command above passed on 2026-09-07 after the form-suffix recovery fix
following v0.22.0. Five former exceptions now agree.
These are a dated measurement, not a permanent inventory:

| Measurement | Result |
|---|---:|
| Declared invalid cases | 298 |
| Spec files scanned | 223 |
| Specs skipped as not implemented | 37 |
| Cases with equal diagnostic sets | 250 |
| Cases with different diagnostic sets | 48 |
| Tree-sitter cases meeting the declared expectation | 298 |
| Re2c cases meeting the declared expectation | 263 |
| Conflicting diagnostic sets | 38 |
| Re2c incomplete diagnostic sets | 7 |
| Re2c extra diagnostics | 3 |
| Re2c silent on these invalid cases | 0 |

The harness also identified 90 legal-claim examples whose absence assertions
belong to the fixture runner. They are not included as invalid-case parity
proof. Zero silent cases does not imply complete diagnostics: re2c still
misses the declared expectation in 35 of the measured cases.

## What this check does not prove

The empty-container regression deletes only the inner material from existing
angle-group, quotation, phonological-group and sign-group reference examples.
It checks clean controls and error detection through file, utterance and
main-tier APIs at two caller origins. The native parser now requires checked
nonempty contents before completing each enclosed construct; an empty tier
body remains a separate case. This is syntax admission, not identical recovery
or diagnostic placement across backends.

The dependent-group regression separately deletes the contents of `%pho`,
`%mod` and `%sin` groups. It retains authored reference bodies (with a `%mod`
label control for the shared phonology grammar), checking file, utterance,
dependent-tier and typed tier-body APIs at two caller origins. The native
phonology/sign combinators require a first item before constructing a group.
All 48 paired observations require clean controls or actual parse errors;
internal failures and later alignment validation cannot satisfy rejection.
This does not impose a nonempty invariant on every tier body or shared AST.

The native file-recovery location control separately checks existing E748,
E750, E256, E766, E342 and E404 examples with LF, CRLF and CR at zero and
nonzero UTF-8 caller origins. Eleven fault cases and three legal controls make
84 observations. The file dispatcher consumes the same lexer owner's token
or line regions instead of dummy locations or reconstructed token previews.
Missing group annotations deliberately use the broader main-tier region;
this is not cross-backend source-location equality or complete span coverage.

The bare-group recovery regression uses E342's original example and the nested
scope legal/deletion pair. All three retain their written content under
serialization through the file, utterance and main-tier APIs at both caller
origins. The invalid inputs still report an error; the legal control is clean.
The native AST no longer uses a fabricated complete retrace or a synthesized
annotation flag. This is a source-preservation contract for those authored
cases, not a demand for identical recovery across backends.

This comparison measures one-way error detection over all authored examples;
diagnostic-code sets are retained only as informational measurements.
It does not establish equal source spans, recovery models, serialization,
valid-input behavior, timing performance, or completeness of the authored
specification. Source-coordinate and recovery regressions have their own
focused tests in the same integration suite.

The invalid-input panic check is separate:

```bash
cargo test -p talkbank-parser-re2c --test integration \
  error_parity::re2c_never_panics_on_invalid_input
```

Performance must be measured for the intended operation and input set with
`benches/parse_comparison.rs`. Earlier speed ratios and corpus-wide estimates
are historical and are not current release guarantees. External corpus
comparisons are investigation evidence, not a validity or release gate.

For the broader release criteria and evidence, see
[1.0 readiness](../../../book/src/contributing/one-zero-readiness.md).

## Fixture sweep admission

The retained GRA, phonology and comment fixture sweeps each require all 200
entries to produce nonempty content; a printed partial count is not success.
An omitted-entry mutation passed all three old sweeps at 199/200 and is rejected
by all three repaired assertions. This checks fixture execution/nonempty-result
admission, not validity, full content preservation or backend equivalence.

## Recovery policy

See the authoritative [Backend compatibility mandate](../../../book/src/architecture/parser-backends.md#backend-compatibility-mandate).
The dated observations above do not require identical recovered models or
diagnostics and do not certify the architecture of every existing recovery path.
