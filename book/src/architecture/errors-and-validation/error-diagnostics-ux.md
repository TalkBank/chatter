# Error Diagnostics UX Standard

**Status:** Current
**Last modified:** 2026-05-30 07:08 EDT

Workspace-wide standard for diagnostic shape, severity, recovery
behavior, span correctness, and integrator output formats. Applies
to the [CHAT-core error system](chat-core-errors.md). Upstream
batchalign-runtime errors follow the same shape and are documented
separately in the `batchalign3` project.

## Objective

Make diagnostics precise, explainable, and actionable for both
developers and non-technical editors, while keeping machine
readability for downstream tools.

## Open concerns

- Message quality across the error catalog is not yet governed by
  one central style standard. Different error codes were authored at
  different times and converge unevenly on the message-quality
  guidance below.

## Canonical Diagnostic Schema

```text
Diagnostic {
  code: String,
  severity: Error | Warning | Info,
  category: Parse | Validation | Alignment | Header | Tier | Internal,
  location: SourceLocation,
  context: ErrorContext,
  message: String,
  suggestion: Option<String>,
  related: Vec<RelatedLocation>
}
```

## Message Quality Standard

Each diagnostic must answer:

1. What failed.
2. Where it failed.
3. Why it likely failed.
4. What to do next.

Avoid internal jargon unless accompanied by user-facing explanation.

## Severity Policy

- `Error`: blocks parse/validation outcome.
- `Warning`: content is usable but has quality/compliance concerns.
- `Info`: optional guidance and migration hints.

Severity must not be overloaded for tooling convenience.

## Recovery Policy: Diagnostic-First, Not Sentinel-First

When parser recovery is required:

- do not invent semantic fallback values to keep type construction
  convenient,
- do not use empty strings or arbitrary enum defaults as recovered
  content.

Instead:

1. Report a diagnostic with expected/actual node context.
2. Preserve span information for tooling and UI.
3. Propagate partial/failure status explicitly.

Any synthetic placeholders that are unavoidable for internal
plumbing must be:

- non-semantic (not exposed as real model content),
- marked internal-only,
- excluded from user-facing diagnostics and serialization.

### Sentinel vs error-variant rule

If an unexpected condition changes semantic trust in parsed content:

- Represent that explicitly as an error-bearing state (enum variant,
  parse-taint flag, or explicit outcome type).
- Never represent it as `None` or a default payload that can be
  mistaken for valid content.

This applies both to parser outputs and to runtime metadata consumed
during validation.

### Diagnostic construction

Use shared constructors/helpers for common diagnostics to reduce
drift:

- span-only diagnostics (`code + severity + span + message`),
- source-backed diagnostics (`code + severity + span + source +
  offending + message`).

Benefits: consistent location/context population, fewer ad-hoc
`ParseError::new(...)` call shapes, simpler migration to richer
miette rendering.

## Error Code Governance

- Central registry under `talkbank-model` (errors module).
- One authoritative description and example per code.
- Deprecated codes remain mapped with explicit migration notes.
- CI check forbids duplicate code definitions or orphaned docs.

## Span and Location Correctness

- All diagnostics use consistent line/column and byte-offset
  definitions.
- Golden tests cover:
  - single-byte and multi-byte UTF-8 content,
  - embedded content offsets,
  - continuation lines and tabs.

## Integrator Output Formats

- Human-readable CLI diagnostics.
- Machine-readable JSON diagnostics.
- LSP diagnostic mapping.

All formats share the same underlying diagnostic schema.

## Acceptance Criteria

- Every emitted diagnostic includes code, severity, location, and
  suggestion policy.
- Error code documentation and runtime definitions are synchronized
  automatically.
- Span correctness is covered by dedicated tests.
- CLI and JSON outputs are contract-tested for schema compliance.
