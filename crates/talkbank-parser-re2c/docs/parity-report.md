# Re2c parser parity

**Status:** Current
**Last updated:** 2026-09-28 20:59 EDT

The [Backend compatibility mandate](../../../book/src/architecture/parser-backends.md#backend-compatibility-mandate)
is the single policy authority for this report. Diagnostic-set agreement is
a measurement, not the definition of correctness or a demand to emulate
tree-sitter recovery.

The re2c backend remains experimental and incomplete. Tree-sitter is the
default production validator; neither backend defines CHAT policy by its
observed behavior. Passing the re2c
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

A reviewed architectural difference may remain deliberately. An exact-code
expectation failure does not by itself prove a missed validity rule. Classify
the semantic outcome and user impact under the compatibility mandate before
changing either implementation or baseline.

## Measured snapshot

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

## Recovery policy

See the authoritative [Backend compatibility mandate](../../../book/src/architecture/parser-backends.md#backend-compatibility-mandate).
The dated observations above do not require identical recovered models or
diagnostics and do not certify the architecture of every existing recovery path.
