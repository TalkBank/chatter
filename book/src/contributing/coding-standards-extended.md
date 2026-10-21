# Coding Standards and Engineering Practices

**Status:** Current
**Last modified:** {{git-dates:page}}

## Objective
Set enforceable, language-specific standards that reduce ambiguity and improve long-term maintainability.

## Global Standards
1. Prefer explicit domain types over ad-hoc strings.
2. Keep parsing, validation, and rendering logic separated.
3. Eliminate magic numbers/strings/paths via named constants and config.
4. Treat generated code as immutable artifacts.
5. Require tests for every bugfix and behavior change.

## Rust Standards
- Follow [Developer Verification Checks](dev-checks.md) and
  [Quality Gates](quality-gates.md): formatting and release-time Clippy have
  different execution scopes; do not add per-edit or commit gates.
- Minimize `#[allow(clippy::...)]`; each allowance needs rationale.
- Prefer small focused modules with clear ownership.
- Public APIs require doc comments with examples and error behavior.
- Field derives admit metadata before emission. `#[semantic_eq(skip)]` and
  `#[span_shift(skip)]` are field-only, single-argument controls; malformed,
  duplicate or misplaced annotations are errors. Equality and structured diff
  share one admitted semantic field plan; span movement has a separate typed
  policy. Preserve recursive bounds, diff paths/context and skipped payloads;
  generate enum locals independently of user field names.
- In parser code, disallow `ErrorSink` + `Option<T>` signatures for fallible parse operations.
  - Use explicit outcome enums or `Result` with structured diagnostics.
  - Preserve streamed diagnostic ownership; explicit production is not
    validation. See [Parser, Model, and API Contracts](../architecture/parser-model-contracts.md).
  - Constructor, wire, policy and public-boundary tests remain necessary
    after typestate changes; see [Testing](testing.md#adding-tests-and-when-not-to).
- For model enums that encode validation state, require `ValidationTagged` derive.
  - Explicit annotation: `#[validation_tag(error|warning|clean)]`.
  - Flags are variant-only and strictly parsed before emission; generic payloads
    do not acquire validation trait bounds.
  - Naming-convention fallback (in `talkbank-derive/src/validation_tagged.rs`):
    variants ending in `Error` → `Error`; variants ending in
    `Warning` OR `Unsupported`, plus a variant named exactly
    `Unsupported`, → `Warning`; otherwise → `Clean`.
- `error_code_enum` consumes an admitted, ordered, UnknownError-complete registry,
  never unchecked attributes. Code literals and exclusive Active/Planned/OptIn
  states have one producer. Reject malformed or duplicate metadata, nonempty
  macro arguments and unsupported generic/where-clause/discriminant declarations;
  preserve exact wire strings, numeric code ordering and generated status policy.

## Grammar Standards
- Grammar rules must map to documented token/category semantics.
- No duplicated symbol sets in free-form literals.
- Every non-obvious precedence/conflict decision must include rationale.

## Spec and Generator Standards
- Spec files must follow strict metadata template.
- Generators must be deterministic and pure with respect to inputs.
- No hardcoded user-specific paths in docs or generated outputs.

## Magic Value Policy
### Disallowed
- Inline path literals tied to local machines.
- Unnamed numeric constants encoding protocol behavior.
- Repeated header/tier string literals across modules.

### Required
- Central constants/modules:
  - path defaults,
  - tier/header prefixes,
  - token categories,
  - formatting policies.

## Review and PR Standards
- PR template must include:
  - subsystem touched,
  - contract impact,
  - generated artifact impact,
  - tests added/updated,
  - docs updated.
- Require at least one reviewer with subsystem ownership for core modules.

## Internal Decision Records
Adopt short ADR format in the book's architecture section:
- context,
- decision,
- alternatives considered,
- consequences,
- rollback path.

## Acceptance Criteria
- Name the actual owner and verification mechanism for each enforceable rule;
  a documented standard is not proof of automatic enforcement.
- Magic values are systematically reduced and tracked.
- Every behavior change includes tests and doc impact assessment.
- Architecture decisions are recorded and discoverable.
