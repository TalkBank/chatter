# Support and Stability Tiers

**Status:** Current
**Last modified:** 2026-06-15 15:00 EDT

## Why this page exists

`TalkBank/chatter` is still a staging repo, so the project needs two truths at
once:

1. what the eventual public surfaces are meant to be, and
2. what support contract this repo can honestly claim **today**.

The rule is simple: **no surface is stable until this repo actually ships that
surface publicly and the release notes say so.** Until then, surfaces are
either preview targets, experimental, or internal-only.

## Tier definitions

| Tier | Meaning |
|------|---------|
| **Stable** | Publicly released from this repo with an explicit support promise, documented install/distribution path, and release notes that describe compatibility expectations. |
| **Preview** | Intended public surface, but not yet a stable promise. APIs, packaging, flags, and workflows may still change quickly. Release notes must call out known gaps and support boundaries. |
| **Experimental** | Source-available evaluation surface with no supported release channel. May change drastically or disappear without deprecation. |
| **Internal** | Not a public product surface. May be unpublished, test-only, generator-only, or otherwise implementation support. |

## Current repo-wide rule

Because this repo has not completed its first public release cutover yet:

- **Stable:** none
- **Preview:** selected first-wave foundations and future public tools, including the desktop app
- **Experimental:** none
- **Internal:** test harnesses, spec tooling, and non-product authority sources

## Surface matrix

| Surface | Path / artifact | Current tier | Current release channel | Notes |
|---------|------------------|--------------|-------------------------|-------|
| `tree-sitter-talkbank` | `grammar/` | Preview | Not yet published from this repo | First-wave crates.io foundation target |
| `talkbank-derive` | `crates/talkbank-derive/` | Preview | Not yet published from this repo | First-wave crates.io foundation target |
| `talkbank-model` | `crates/talkbank-model/` | Preview | Not yet published from this repo | First-wave crates.io foundation target |
| `talkbank-cache` | `crates/talkbank-cache/` | Preview | Not yet published from this repo | First-wave crates.io foundation target |
| `talkbank-parser` | `crates/talkbank-parser/` | Preview | Not yet published from this repo | First-wave crates.io foundation target |
| `talkbank-parser-re2c` | `crates/talkbank-parser-re2c/` | Preview | Not yet published from this repo | First-wave crates.io foundation target because `talkbank-transform` depends on it at runtime |
| `talkbank-transform` | `crates/talkbank-transform/` | Preview | Not yet published from this repo | First-wave crates.io foundation target |
| `send2clan` | `crates/send2clan/` | Preview | Held back with `publish = false` | Wave 1B hold-back until support/install contract is documented |
| `talkbank-cli` / `chatter` | `crates/talkbank-cli/` | Preview | Held back with `publish = false`; cargo-dist workflow exists | Flagship user surface, but this repo is not yet the public binary source of truth |
| `talkbank-lsp` | `crates/talkbank-lsp/` | Preview | Held back with `publish = false` | Public editor/integrator target; release/install contract still incomplete |
| Chatter Desktop | `apps/chatter-desktop/` | Preview | Build-from-source; ships in the coordinated release with the CLI | First-wave release surface alongside the CLI |
| Parser test harness | `crates/talkbank-parser-tests/` | Internal | `publish = false` | Shared internal regression gate |
| Spec generators | `spec/tools/`, `spec/runtime-tools/` | Internal | `publish = false` | Internal generation/runtime tooling |
| Spec authority | `spec/` | Internal | Not a product artifact | Source of truth for constructs, errors, and symbols |

## Release-note requirements

Every public release note, release announcement, or publication checklist for a
surface must state all of the following:

1. **Tier**: stable or preview
2. **Channel**: crates.io, GitHub Releases, or source-only
3. **Support boundary**: what users should rely on and what is still incomplete
4. **Known hold-backs**: closely related surfaces that remain unpublished or unsupported

If a release note cannot state those four things clearly, the surface is not
ready to be described as released.

## Practical consequences

- Do not describe any surface in this repo as **stable** until the first public
  release actually happens from this repo.
- Preview surfaces must say **preview** in user-facing docs when the packaging
  or support story is still incomplete.
- Experimental surfaces must tell users what to use instead for production work.
- Internal surfaces should stay unpublished and should not be marketed as user
  products.
