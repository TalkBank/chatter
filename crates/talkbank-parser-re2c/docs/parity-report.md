# Re2c parser parity

**Status:** Current
**Last updated:** 2026-09-24 00:21 EDT

The re2c backend does not yet match tree-sitter on every declared invalid
example. Tree-sitter remains the CHAT validity authority. Passing the re2c
regression gate means its remaining differences match an explicit baseline;
it does not mean the backends agree or satisfy every specification.

## Reproduce the invalid-example comparison

From the repository root:

```bash
cargo test -p talkbank-parser-re2c --test integration \
  error_parity::backends_diverge_only_where_recorded -- --nocapture
```

The harness reads declared examples from `spec/errors/`, parses and validates
with both backends, and reports two different questions:

- Do the backends emit the same diagnostic-code sets?
- Does each backend satisfy the example's declared expectation?

Agreement alone cannot establish correctness: both backends could miss the
same rule. Different code sets also require adjudication; the larger set is
not automatically the better answer.

The observed differences are reconciled with
[`KNOWN_DIVERGENCES`](../tests/integration/error_parity/baseline.rs).
An unrecorded difference fails the gate, and an obsolete baseline entry must
be removed when a fix makes the backends agree. Do not add a baseline entry
merely to make a failing test pass.

## Measured snapshot

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

This comparison measures diagnostic-code sets for declared invalid examples.
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

## MISSING-Token Recovery Policy

Tree-sitter can insert a zero-length MISSING token to continue parsing a
malformed construct. Recovery has two obligations: preserve the useful model
structure and report the defect. A recovered node is not evidence that its
source was valid.

Re2c recovery belongs in the lexer or parser rule that owns the construct.
Carry the recovered structure into the model and emit the corresponding
diagnostic through the caller's error sink. Do not infer the construct by
scanning a generic error's source text afterward, or silently invent a valid
annotation to keep the parse moving.

For example, a group without its required postfix marker needs an explicit
recovery decision and a diagnostic. Matching the recovered model alone is
insufficient: `SemanticEq` does not compare diagnostic streams. Test model
recovery and error locations separately against the declared specification.
The baseline above records remaining diagnostic differences; it is not a
waiver of either recovery obligation.
