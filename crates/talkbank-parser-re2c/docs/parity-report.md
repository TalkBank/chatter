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

### Retained specification typed dependent bodies

`error_parity::tier_population::retained_spec_typed_bodies_preserve_fragment_error_detection`
selects parser-owned body extents using each retained tier's model label. This
includes explicit raw recovery when body geometry exists. It never derives body
boundaries from source prefixes or substitutes an empty body for missing geometry.
Each selected fragment owns its API identity; that identity is also part of the
comparison key, so equal text used by different typed methods cannot share a result.

The finite authored deck supplies 3,195 body observations for nine API families:
MOR, GRA, PHO (including MOD), SIN, ACT, COD, COM, ADD and WOR. All 2,195 distinct
comparisons pass with 272 required detections; 10,585 identical observations reuse
results. Nine known empty COM bodies are measured. Three MOR and 33 WOR observations
have no body extent and are reported separately. EXP, GPX, INT, SPA and SIT have
no body witnesses in this deck; their existing reference and isolated controls
remain separate evidence.

Both overloads run at local/document origins under three newline transports,
with implicit or full document-option context. These typed-body observations
are parse-only, including caller-range checks and internal-failure refusal.
No new parser defect surfaced. Passing this selection does not prove complete
typed-API coverage, shared validation, omitted-source recovery or every fault
within a multi-fault body.

### Retained specification word fragments

`error_parity::tier_population::retained_spec_words_preserve_fragment_error_detection`
selects original word spans from every retained main tier in the authored error
deck. The shared structural walk includes nested and interposed words, annotated
words and the original word of a replacement. It does not synthesize words from
replacement text or use the alignment walk, which excludes interposed speech.

The 11,669 source observations pass 7,239 distinct comparisons with 1,070 required
detections; 39,437 identical observations reuse results. Both word APIs run at
local/document origins under authored, CRLF and CR transports. Shared word
validation runs separately under standalone defaults and the invocation's CA
policy. The document owner, full context key, source bounds and internal-failure
refusal are shared with the tier populations below.

No new parser defect surfaced. These are retained lexical slices, not all source
tokens or every word rejected before model construction. Enclosing code-switch
annotations and participant/language declarations are not carried into standalone
word validation. Parent annotations, replacement text, whole-file relationships
and isolated faults outside this finite selection retain their own obligations.

### Retained specification main-tier fragments

`error_parity::tier_population::retained_spec_main_tiers_preserve_fragment_error_detection`
selects every retained main tier from the authored error deck under authored,
CRLF and CR transports. The parsed-document owner binds source bytes, origin
and option context. Both main-tier APIs run at local and document origins.
Main-tier model validation runs as a separate phase under the corresponding
standalone policy, with CA enabled for explicit CA context. Participant/language
declarations, whole-file relationships and alignment are outside this policy.

All 12,691 distinct comparisons pass, including 4,620 required detections.
The 6,476 retained source observations request 25,904 comparisons; 13,213
identical observations reuse results under the complete invocation key. Returned
payload spans stay within their caller's input; rejection needs an error and
internal failure never satisfies detection.

The initial parse-only comparison exposed known phase differences in E233,
E245, E253 and E306: the primary parser can reject syntax whose retained native
model is rejected by the shared validator. The phased check respects those
architectures. Existing isolated parse-only controls remain unchanged. This
population found no new parser defect and proves neither complete recovery nor
whole-language correctness; source omitted by whole-file recovery is unmeasured.

### Retained specification dependent fragments

`error_parity::tier_population::retained_spec_dependent_tiers_preserve_fragment_error_detection`
selects every retained dependent-tier extent
from the complete authored error deck, including legal and deferred examples,
under authored, CRLF and CR transports. A parsed-document owner supplies original
bytes, source locations and option context together. Both generic dependent-tier
adapters run at local and document origins. Explicit calls retain the document's
full option context; implicit calls retain their own default policy.

All 5,604 distinct comparisons pass, with 822 required error detections. The
3,729 source-tier observations request 14,916 comparisons; 9,312 identical
observations reuse results keyed by exact bytes, origin and complete invocation
policy. Context equality retains unsupported option text, order and repetitions.
Returned payload spans stay inside the caller's input, diagnosed rejection is
required, and internal tool failures refuse measurement. No new parser defect
was found in this population.

This is a parse-only generic-fragment check. It does not apply model validation,
alignment, whole-document relationships or each example's file-rule profile.
Tiers omitted by whole-file recovery are outside this selection. It neither
requires equal recovery models nor proves every malformed tier or every isolated
fault is detected. Typed dependent-body APIs retain their separate controls.

### Retained specification header fragments

`error_parity::header_population` selects original-source header extents from
every authored example's retained whole-file model under authored, CRLF and CR
transports. Checked source slices keep each header's bytes and document origin
together. Both header adapters run at local and document origins, with implicit
and explicit empty fragment context. Shared `Header::validate` runs separately
under its default context: an error may be reported by either phase. Header
payload spans must stay within the caller's range; rejected parsing needs an
error and internal failures never satisfy detection.

The finite population retains 29,267 header observations and passes all 7,944
distinct comparisons, with 1,812 required detections. A typed probe key contains
the complete bytes, origin and API invocation under this fixed validation
policy; 109,124 identical observations reuse those comparisons. The population exposed a
Tree-sitter wrapper defect on E307's non-Latin participant example under CR:
the synthetic leading LF extended the caller's final CR into a CRLF token,
placing a recovered header one byte outside the input. Wrapper assembly now
admits a separated suffix before computing capacity and constructing its source.
The caller's bytes and checked rebasing remain intact.

These are retained header fragments, not every malformed source line; a header
not retained by whole-file parsing is outside this selection. Header validation
does not apply whole-document relationships or the example's file-rule profile.
This check neither requires identical recovery models nor replaces the separate
valid-reference semantic comparison. Error reports preserve explicit unlocated
diagnostics rather than panicking while formatting failed measurements.

### Annotation-position admission

The 28 authored `E316_annotation_positions.md` examples isolate word, event,
action, group and quotation annotation positions, with seven legal controls.
Before the repair, 14 invalid examples produced no native error under each of
the three newline transports. The full invalid-example comparison now passes.

Words admit one optional replacement before base annotations; other hosts admit
only scoped annotations and retrace markers, with required whitespace. The
native AST stores the replacement separately and carries only scoped annotations
after retrace folding. Lowering no longer filters arbitrary bracket constructs
or searches for the first replacement, so misplaced constructs remain available
to the existing orphan-error recovery path. The shared lexical-word API also
rejects attached annotation syntax instead of silently discarding it.

The new fragment regression checks the authored main tier through main-tier and
utterance APIs under three newline transports at two caller origins, plus bare
and annotated word controls. Valid reference semantics and existing shared-model
snapshots remain unchanged. Five native AST snapshots change representation;
the experimental carrier API change is documented in the changelog. This closes
the demonstrated silent-loss cases, not annotation-language completeness or
cross-backend diagnostic-code equality.

### Original whole-utterance fragments

`equivalence_tests::reference_utterances` selects every utterance from the
complete reference corpus under authored, CRLF and CR transports. Each slice
starts at its original main tier and includes all attached dependent tiers;
no utterance is reconstructed from rendered text. A checked reference-document
owner keeps the source, parsed model and option context together.

Both public utterance adapters must return a clean model with the same meaning
as the whole-file utterance, at local and document origins. The current finite
population passes 10,164 comparisons over 2,541 utterance observations, including
1,962 attached dependent tiers, six multiline continuations and 396 observations
with option context. These counts include the three transports. Missing source
extents, absent boundary witnesses and an empty comparison population fail.

Interstitial headers remain whole-file lines in this population; this contract
does not claim source extents for builder-attached `preceding_headers`. It
compares semantic content, not exact source-location identity or invalid-input
recovery. The test belongs to the existing `equivalence_tests` family; these
correctness results do not update an older source-bound coverage measurement.

### Numeric phonology pauses

The fifth authored examples in E737 and E738 include `(1:02.5)` on `%mod`
and `%pho`. These are phonological words, including the numeric colon, not
main-tier pause tokens to discard. The native lexer now admits the complete
numeric spelling without admitting arbitrary colons in phonological words.
The parsed word's nonempty segment list has a single construction path; the
old empty-word pause placeholder and filtering path are removed.

`equivalence_tests::phonological_pauses::authored_numeric_phonology_pauses_agree`
passes 18 comparisons: both files under authored/CRLF/CR transport at zero and
nonzero origins, both original-source tier fragments at two origins, and the
numeric word at two origins. File comparisons include the declared validation
selection and filename. This bounded regression is not construct completeness,
source-location identity or release qualification. Existing phonology debug-AST
snapshots remain unchanged.

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
The original-main-tier comparison additionally selects every main tier from the
checked reference corpus using its model-owned source span. It preserves physical
continuations and supplies the file's option flags to both fragment adapters.
Each case must parse through the native low-level API, agree across adapters,
and agree with the admitted whole-file main tier. No percentage threshold or
missing-directory skip permits partial success. This replaces the ignored
physical-line smoke test, which inspected only six directories and accepted
more than 90% of its selected lines.

The typed-reference fragment comparison adds ID headers and the dedicated PHO,
SIN, ACT, COD, COM, EXP, ADD, GPX, INT, SPA, SIT and WOR body APIs. It selects
model-owned spans from each reference document in authored, CRLF and lone-CR
form and retains the document's option flags. Each exact slice is compared at
zero and its original document offset. The population must contain every one
of the thirteen routes before comparison; missing routes, empty or invalid
source slices and recovery tiers refuse admission. PHO and MOD bodies both
exercise the PHO body API; this does not test a MOD-specific fragment method.
No parsed value is serialized to manufacture these source witnesses.

The separate `equivalence_tests::reference_leaf_fragments` population selects
participant entries, MOR words, GRA relations and complete PHO words (including
compounds) through generated typed nodes in the producer-bound reference CST.
Its source traversal retains node/text identity and propagates unreadable ranges;
it does not reconstruct leaf text from models or assign invented model spans.
Every route must have source witnesses before comparisons begin. A control removes
each route independently and requires population admission to fail.

The full source is transported before extracting these leaves. Each slice is
compared through both backends at zero and its document origin, using implicit
and document-option-context calls separately. The two invocation policies are
not asserted to have identical semantics. These controls establish finite peer
agreement on original reference leaves; they do not measure every possible leaf
shape or replace independent expected-model and fault/refusal controls.

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
  trait_contracts:: parser_fixtures::convert_ \
  fragment_corpus::contextual_fragment_contracts:: \
  fragment_corpus::invalid_timing_states_from_specs_rebase_to_document_coordinates \
  fragment_corpus::reference_dependent_tier_fragments_preserve_source_models \
  fragment_corpus::spec_documents_preserve_utterance_adapter_recovery_under_rebasing \
  fragment_corpus::dependent_composition_contracts:: --nocapture
```

The optional native `trait_contracts` controls admit a model only after observing
an empty diagnostic batch. A returned recovery model is insufficient. The clean
file control uses the basic reference conversation, and an authored E522 example
checks that a recovered file with diagnostics cannot enter that clean state.
This is parse admission, not full validation or complete trait-surface coverage.

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
