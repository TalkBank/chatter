# Coding Standards and Engineering Practices

**Status:** Current
**Last updated:** 2026-05-21 08:38 EDT

## Objective
Set enforceable, language-specific standards that reduce ambiguity and improve long-term maintainability.

## Global Standards
1. Prefer explicit domain types over ad-hoc strings.
2. Keep parsing, validation, and rendering logic separated.
3. Eliminate magic numbers/strings/paths via named constants and config.
4. Treat generated code as immutable artifacts.
5. Require tests for every bugfix and behavior change.

## Rust Standards
- Enforce formatter and clippy in CI.
- Minimize `#[allow(clippy::...)]`; each allowance needs rationale.
- Prefer small focused modules with clear ownership.
- Public APIs require doc comments with examples and error behavior.
- In parser code, disallow `ErrorSink` + `Option<T>` signatures for fallible parse operations.
  - Use explicit outcome enums or `Result` with structured diagnostics.
  - Guardrail script: `scripts/check-errorsink-option-signatures.sh`.
- For model enums that encode validation state, require `ValidationTagged` derive.
  - Explicit annotation: `#[validation_tag(error|warning|clean)]`.
  - Naming-convention fallback (per
    `crates/talkbank-derive/src/validation_tagged.rs:118-123`):
    variants ending in `Error` → `Error`; variants ending in
    `Warning` OR `Unsupported`, plus a variant named exactly
    `Unsupported`, → `Warning`; otherwise → `Clean`.

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
- Coding standards are documented once and enforced automatically.
- Magic values are systematically reduced and tracked.
- Every behavior change includes tests and doc impact assessment.
- Architecture decisions are recorded and discoverable.
