# Error Diagnostics UX Standard

**Status:** Current
**Last modified:** {{git-dates:page}}

Workspace-wide standard for diagnostic shape, severity, recovery
behavior, span correctness, and integrator output formats. Applies
to the [CHAT-core error system](chat-core-errors.md). External application
errors belong to their owning projects; this page does not define their schema.

## Objective

Make diagnostics precise, explainable, and actionable for both
developers and non-technical editors, while keeping machine
readability for downstream tools.

## Open concerns

- Message quality across the error catalog is not yet governed by
  one central style standard. Different error codes were authored at
  different times and converge unevenly on the message-quality
  guidance below.

## Core evidence and output contracts

`ParseError` is the core evidence type. It carries a typed code, severity,
source location, optional context, secondary labels, message, optional
suggestion and optional help URL. See the [core types](chat-core-errors.md#core-types)
for the actual fields; there is no core `Info` severity or `category` field.

Original source evidence and checked display context are distinct types.
Raw miette rendering borrows the diagnostic's current `ErrorContext`. Editing
or replacing that public context therefore changes the next render; there is no
independent cached source copy to invalidate. Snippet line-offset overflow is a
rendering error, not a wrapped or fabricated line number. This does not admit
the context as original-source evidence: checked `SourceDiagnostic` attachment
still owns full-source presentation and its separate immutable projection.

Consumer projections are not identical serializations of `ParseError`:
CLI NDJSON, audit records and LSP diagnostics have their own shapes and
coordinate conventions. The [diagnostic contract](../../chatter/integrating/diagnostic-contract.md)
owns the CLI wire format and source-presentation boundary.

## Message Quality Standard

Each diagnostic must answer:

1. What failed.
2. Where it failed.
3. Why it likely failed.
4. What to do next.

Avoid internal jargon unless accompanied by user-facing explanation.

## Severity Policy

- `Error`: a CHAT finding blocks complete admission under the selected policy.
  It need not prevent recovery from producing a model. An internal failure
  determines neither validity nor invalidity of the input.
- `Warning`: a reportable finding that does not itself block admission;
  it does not prove that the rest of the content is valid or usable.

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
- excluded from serialized semantic content and never presented as real
  source evidence. Report the underlying internal failure honestly.

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

- `spec/codes/error-codes.toml` owns identifiers, descriptions, diagnostic
  kinds, implementation status and retired numbers.
- `spec/errors/` owns examples and claims; a code can need multiple examples,
  including legal controls and interactions with other findings.
- Generated Rust definitions and error pages are outputs, not editing targets.
- Follow [Spec Workflow](../../contributing/spec-workflow.md) for regeneration
  and focused verification. Generated currency does not prove rule correctness.

## Span and Location Correctness

- Keep original document byte ranges distinct from snippet-relative display
  spans. Use the producing source snapshot, not a later reread.
- Source-location line/column metadata is one-based and currently counts
  bytes; LSP uses zero-based UTF-16 positions. Use the owning conversions.
- Reject invalid display ranges rather than clamping them or inventing a
  location. Missing context is an explicit presentation outcome, not a new
  validity verdict. Bounds checks alone cannot prove source identity.
- Review boundary tests for:
  - single-byte and multi-byte UTF-8 content,
  - embedded content offsets,
  - continuation lines and tabs.
  - zero-width EOF, unavailable context and invalid UTF-8 boundaries.

## Integrator Output Formats

- Human-readable CLI diagnostics.
- Machine-readable JSON diagnostics.
- LSP diagnostic mapping.

All consume core findings, but their wire representations differ. Do not
assume CLI NDJSON carries every core field or copy byte columns into LSP ranges.

## Review criteria

These are obligations for a diagnostic change, not a claim that every code
and consumer has already passed an exhaustive review:

- Preserve code, severity and original evidence; make unavailable source
  context explicit and offer a suggestion only when supported.
- Error code documentation and runtime definitions are synchronized
  automatically.
- Verify affected span, recovery and presentation boundaries with dedicated tests.
- Verify affected consumer output against its own contract, including failure
  and unavailable-context cases, rather than a fictional universal schema.
