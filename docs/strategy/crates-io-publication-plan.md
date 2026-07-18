# crates.io Publication Plan (v1.0.0)

**Last modified:** 2026-07-17 23:20 EDT

Worked plan for publishing chatter's library crates to crates.io,
simultaneous with the v1.0.0 release (release-board precondition P2).
This is planning material; the actual publish is gated on Franklin's go
and the release freeze. Nothing here is executed autonomously.

## Current readiness: GREEN foundation

`scripts/release/check-foundation-publication-readiness.sh` passes
(verified 2026-07-17, `--allow-dirty`), including a real
`cargo publish --dry-run` of `tree-sitter-talkbank`. It validates, for
the first-wave set: `publish` flags, required metadata
(`repository`, `homepage`, `keywords`, `categories`, `readme` and that
the readme file exists), that no first-wave crate has a runtime
dependency on a held-back crate, and that the first-wave list is in
correct topological order. `just crates-io-foundation-check` runs it.

## The first wave (publish order)

Topologically ordered so each crate's runtime dependencies are already
on crates.io when it publishes:

1. `tree-sitter-talkbank` (no internal deps; the only one that can
   fully `--dry-run` before the rest exist on the registry)
2. `talkbank-derive`
3. `talkbank-model`
4. `talkbank-cache`
5. `talkbank-parser`
6. `talkbank-parser-re2c`
7. `talkbank-transform`

This set gives an external consumer the full CHAT library surface: the
typed model, both parsers, validation, transform, and CHAT<->JSON. It
is what `pylangacq` / `rustling`-style downstreams need to depend on
chatter directly.

## Held back (publish = false), deliberately

- `chatter` (the CLI): shipped as a signed binary via cargo-dist
  GitHub Releases, not as a crate. **Open decision (Franklin):** also
  publish the `chatter` crate to crates.io so `cargo install chatter`
  works? That is a separate, later wave if wanted; it is not required
  for the library-consumer goal and adds a maintenance surface.
- `talkbank-lsp`: standalone LSP artifact ships in the cargo-dist
  release; crate publication is a later decision.
- `send2clan`: macOS/Windows CLAN-app FFI; niche and platform-specific,
  no library consumer needs it. Stays unpublished.
- `talkbank-llm`: the LLM client. NOT in the wave and correctly so: it
  is a leaf that depends ON `talkbank-transform`, nothing in the wave
  depends on it, and its API is not a general CHAT-library surface.
  (It currently lacks a `readme`; harmless while unpublished, and only
  needs one if a future decision publishes it.)
- `talkbank-parser-tests`, `xtask`, `chatter-desktop`, `grammar`
  tooling: test/dev/app crates, never published.

## Open items before the real publish

Two real pre-publish steps the local check structurally cannot settle,
plus one item already verified done (item 3). None is a blocker today;
items 1 and 2 execute in order on the release.

1. **Inter-crate version requirements track the release version.** The
   `[workspace.dependencies]` pins currently require `talkbank-* =
   "0.3.2"` (and `tree-sitter-talkbank = "0.1.0"`) while the workspace
   is at `0.3.6`. Caret resolution makes this work today (0.3.6
   satisfies ^0.3.2), but for a clean 1.0 publish every inter-crate
   requirement should state the version actually being published. Bump
   these pins as part of the 1.0 version bump. `tree-sitter-talkbank`
   is independently versioned at `0.1.0`; decide whether it joins the
   1.0 line or keeps its own SemVer (grammar crates commonly version
   independently, which argues for keeping it separate).
2. **Dev-dependency-on-unpublished-crate hazard.** Two first-wave
   crates carry a `[dev-dependencies]` edge, with a version, onto a
   crate that is not (yet) on the registry at their publish moment:
   `talkbank-derive` dev-depends on `talkbank-model` (the intentional
   test-only cycle), and `talkbank-parser` dev-depends on the
   never-published `talkbank-parser-tests`. The readiness check only
   inspects runtime deps, so it does not surface these. Standard
   mitigation is a path-only dev-dependency (no version), which cargo
   omits from the published package; the alternative is restructuring
   so the dev edge disappears. This must be resolved before `derive`
   and `parser` publish, and validated by the real ordered
   `cargo publish --dry-run` as each crate lands (the only authoritative
   test, per the readiness script's own note). Flagged as a hazard to
   validate, not a confirmed blocker: it cannot be dry-run-tested
   locally because of the chicken-and-egg with unpublished siblings.
3. **rustdoc completeness: DONE, enforced.** crates.io publishes
   rustdoc to docs.rs; the house rule is that a reader understands the
   domain from type definitions alone. Verified 2026-07-17: all seven
   first-wave crates carry `#![deny(missing_docs)]` at their crate root
   (`talkbank-model` lib.rs:52, ..., `tree-sitter-talkbank`
   bindings/rust/lib.rs:2), so any undocumented public item fails the
   build. No gap exists and none can be introduced. Nothing to do
   before publish.

## Publish-day sequence (when unfrozen and approved)

1. Bump inter-crate version requirements to the release version (item 1).
2. Resolve the dev-dep hazard (item 2); re-run the readiness check.
3. Publish crates strictly in the first-wave order above, running
   `cargo publish --dry-run -p <crate>` immediately before each real
   `cargo publish -p <crate>`, so registry resolution is verified live
   at each step.
4. Tag the release (cargo-dist ships the CLI + desktop binaries in the
   same release).
5. Smoke-test: a scratch crate that depends on `talkbank-model` from
   crates.io builds and parses a `.cha` file.
