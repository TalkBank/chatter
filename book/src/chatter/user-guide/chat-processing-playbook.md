# CHAT Processing Playbook for Editors and Analysts

**Status:** Current
**Last updated:** 2026-03-24 00:01 EDT

## Objective
Provide practical guidance for non-compiler users who create, edit, and validate CHAT files,
with emphasis on error interpretation and correction workflow.

## Who This Is For
- Transcript editors,
- corpus curators,
- QA reviewers,
- linguists using tooling outputs but not parser internals.

## Core Editing Workflow
1. Open file in editor with CHAT diagnostics enabled.
2. Run validation (single file first, then batch).
3. Fix highest-severity structural issues first (headers, tier markers, unmatched delimiters).
4. Re-run validation and inspect warnings.
5. Only then address style and normalization suggestions.

## Error Triage Heuristic
- Errors at file start: likely header formatting or encoding issues.
- Errors at tier prefix: likely malformed `*`/`%` tier syntax.
- Errors inside words: likely symbol, marker, or annotation boundary issues.
- Repeated same error class: likely one systemic rule violation pattern.

## Fast Interpretation Guide
- `Error`: parser/validator could not accept structure; must fix.
- `Warning`: valid but suspicious or non-canonical; review strongly recommended.
- `Info`: advisory normalization or convention hints.

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
4. Re-run and confirm error count decreases monotonically.
5. Save run report for audit trail.

## Collaboration Workflow with Developers
When reporting parsing issues, include:
- exact file path,
- minimal excerpt around failing span,
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
