# spec, CHAT Specification

**Last modified:** 2026-08-21 07:05 EDT

## Overview

Markdown specification files define valid constructs and error cases for CHAT.
`spec/tools/` turns these specs into tree-sitter corpus tests, Rust tests, and
documentation. Corpus candidate selection and live parser/model validation live in the
sibling `spec/runtime-tools/` crate.

**Specs are the source of truth.** Generated artifacts should never be edited
by hand.

## Structure

```
spec/
├── constructs/           Valid CHAT examples (164 specs)
│   ├── header/           Header constructs
│   ├── main_tier/        Main tier constructs
│   ├── tiers/            Dependent tier constructs
│   ├── utterance/        Utterance-level constructs
│   └── word/             Word-level constructs
├── errors/               Error specs (197 files, 181 error codes)
├── symbols/              Shared symbol registry (JSON + generators)
├── tools/                Core generator crate in the spec workspace
│   ├── src/bin/          Spec-to-artifact entry points
│   └── templates/        Tera templates for wrapping test fragments
├── runtime-tools/        Candidate selection + live parser/model validation
│   └── src/bin/          Live parser/model-aware entry points
└── docs/                 Format reference and guides
    ├── ERROR_SPEC_FORMAT.md   ← Comprehensive spec format reference
    └── WRITING_ERROR_SPECS.md ← Quick workflow guide
```

## Key Commands

```bash
# Regenerate every committed artifact derived from spec/: the tree-sitter
# corpus tests, the Rust parser test bodies, the validation fixture corpus and
# its manifest.json, and the DiagnosticKind registry.
just spec-gen

# Report which of them are stale, writing nothing. This is what the gate runs.
just spec-check

# (docs/errors/ is a spec-gen artifact; no separate command.)

# Do the error specs' examples produce the codes they declare?
just spec-validate-examples

# Which codes have specs, and which specs demonstrate their own code?
just spec-coverage
```

## Current coverage

**Run `just spec-coverage` and `just spec-status`.** This section used to hold a
five-row table of counts, and on 2026-08-21 every one of them was wrong:
construct specs 164 against 134, error specs 197 against 238, codes covered
181/181 against 224/224, examples 169 against 180, stubs 12 against 4. It had
been wrong long enough that the doc-dates ratchet listed this file as known
stale, and a sweep that restamped the header without re-grounding the body
briefly made it certify as CURRENT, which is worse than being listed as stale.

Numbers that two commands derive do not belong in prose.

## Workflows

See `docs/ERROR_SPEC_FORMAT.md` for the complete format reference, including
metadata fields, layer semantics, code block info strings, and template usage.

See `docs/WRITING_ERROR_SPECS.md` for the practical step-by-step workflow.
See `docs/CURATION_WORKFLOW.md` for the mine -> curate -> generate workflow for construct specs.

## See Also

- `tools/CLAUDE.md`: Core generator crate details
- `runtime-tools/`: Runtime-aware spec tooling
- `CLAUDE.md` (spec directory), AI assistant guidance
- `../crates/talkbank-parser-tests/CLAUDE.md`: Parser test crate

---
