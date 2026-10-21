# CHAT Processing Playbook for Editors and Analysts

**Status:** Current
**Last modified:** {{git-dates:page}}

## Objective
Provide practical guidance for non-compiler users who create, edit, and validate CHAT files,
with emphasis on error interpretation and correction workflow.

## Who This Is For
- Transcript editors,
- corpus curators,
- QA reviewers,
- linguists using tooling outputs but not parser internals.

## Core Editing Workflow

Start with the [practice tutorial](quick-start.md) if this is your first use.

1. Keep an unchanged original or a version-controlled checkpoint.
2. Open the file in an editor with CHAT diagnostics enabled, and validate it.
3. Compare each diagnostic with the documented rule and intended transcription.
   Investigate a possible tool defect before changing data just to silence it.
4. Resolve structural issues first (headers, tier markers, unmatched delimiters).
5. Review the diff, revalidate, and inspect warnings. Treat normalization as a
   separate, reviewable change.

## Error Triage Heuristic
- Errors at file start: likely header formatting or encoding issues.
- Errors at tier prefix: likely malformed `*`/`%` tier syntax.
- Errors inside words: likely symbol, marker, or annotation boundary issues.
- Repeated same error class: likely one systemic rule violation pattern.

## Fast Interpretation Guide
- `Error`: the selected parser/validator checks reject the file; resolve the
  data issue or report a confirmed tool defect.
- `Warning`: valid but suspicious or non-canonical; review strongly recommended.

Validation uses error and warning severities, not a separate `Info` level.
Passing format checks does not verify transcription accuracy or recording identity.

## Common Fix Recipes
- Header spacing problems:
  - Ensure expected separators and avoid accidental tabs/spaces drift.
- Unclear language/form markers:
  - Confirm `@s` usage and suffix ordering with house style guide.
- Duration/annotation confusion:
  - Verify bracketed annotation form and avoid malformed punctuation.
- Dependent tier attachment issues:
  - Ensure `%` tiers follow intended main tier and keep indentation consistent.

## Batch Validation Workflow
1. Validate a small sample first.
2. Group failures by error code.
3. Fix by pattern, not file-by-file random order.
4. Review the transformation on affected files, then revalidate. Fixing an early
   structural error can expose later errors; a monotonic count is not a requirement.
5. Save the command, tool version, input revision and run report for an audit trail.

## Collaboration Workflow with Developers
When reporting parsing issues publicly, include:

- a minimal synthetic file that reproduces the issue, without private paths,
  participant identities or restricted transcript content,
- the command and `chatter --version` output,
- observed diagnostic code/message,
- expected behavior (if known).

This reduces back-and-forth and speeds defect triage.

## Quality Checklist Before Publishing Corpus Updates
- No unresolved error-level diagnostics.
- Warning classes reviewed and accepted or fixed.
- Participant headers and IDs internally consistent.
- Roundtrip serialization check passes for representative samples.
- Changelog note recorded for major normalization edits.

## Training Recommendations
- Maintain short examples for each common error class.
- Provide editor cheat sheet for tier prefixes and marker syntax.
- Run periodic QA calibration sessions across editors.
