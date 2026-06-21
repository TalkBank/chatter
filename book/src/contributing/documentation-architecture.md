# Documentation Architecture

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

## Principle: Centralized Book + Subsystem Satellites

User-facing and contributor-facing prose lives in **mdBook**
(`book/`). The repo-level `docs/` directory holds operator-facing
material (release contract, versioning, code-signing, platform
support, validation feature flags). Maintainers can also generate a
local error-reference tree under `docs/errors/` while working on
diagnostics, but that output is not the canonical checked-in docs
surface. Subsystem-specific working docs stay in place
only when tightly coupled to files in that directory.

```mermaid
flowchart TD
    main["book/ (the unified Chatter mdBook)\nSurfaces: chatter, chat-format, architecture, contributing\nAudiences: users, integrators, contributors"]
    spec["spec/docs/\nSpec authoring guides"]
    errors["docs/errors/\nOptional local generated error reference"]
    api["cargo doc\nRust API docs (auto-generated)"]

    main -->|"links to"| spec
    main -->|"links to"| errors
    main -.->|"complements"| api
```

## Where Documentation Goes

| Content type | Location | Examples |
|---|---|---|
| User guides, CHAT format reference | `book/src/chatter/user-guide/`, `book/src/chat-format/` | CLI usage, validation errors |
| Architecture and design | `book/src/architecture/` | Parsing, data model, concurrency, memory |
| Contributor workflows | `book/src/contributing/` | Grammar workflow, testing, coding standards |
| Integrator contracts | `book/src/chatter/integrating/` | JSON schema, diagnostic contract |
| Technical reference and audits | `book/src/` (Technical Reference section) | Parity audits, UTF-8 audit, risk register |
| Spec authoring guides | `spec/docs/` | Error spec format, curation workflow |
| Generated error docs | `docs/errors/` | Optional local output from `gen_error_docs`; source of truth stays in `spec/errors/` |
| Historical/archived docs | project archive | Old audits, superseded proposals |
| AI assistant context | `CLAUDE.md` files (per repo/subdir) | Not documentation for humans |

## Rules

1. **One canonical page per topic.** No duplicate coverage across locations.
2. **No crate-level `docs/` directories.** Architectural explanations go in the book.
   Crate API docs come from `///` doc comments via `cargo doc`.
3. **Satellites stay only when the audience is editing files in that directory.**
   Spec authors need `WRITING_ERROR_SPECS.md` next to their specs. Everyone else
   reads the book.
4. **Generated docs are build artifacts.** Never hand-edit `docs/errors/`. If you
   need that local reference set, regenerate it with `gen_error_docs`.
5. **Historical docs go to project archive.** Don't keep old audit logs,
   investigation notes, or superseded proposals in the public repo.

## One unified book

There is one mdBook for this repo at `book/`,
titled "Chatter, TalkBank CHAT Toolchain", organized by audience-first sections
under `book/src/`:

| Section | Audience | Content |
|---|---|---|
| `book/src/chatter/` | chatter CLI users + integrators | CLI reference, library usage, JSON contracts |
| `book/src/chat-format/` | All users + integrators | CHAT format reference (headers, tiers, symbols) |
| `book/src/architecture/` | All devs | Cross-surface architecture, parser/grammar/data-model design |
| `book/src/contributing/` | Contributors | Setup, testing, coding standards, dev checks |

One `book.toml` and one `SUMMARY.md` for the whole tree. Cross-section
links resolve as ordinary in-book paths.
