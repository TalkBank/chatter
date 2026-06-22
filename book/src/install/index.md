# Install

**Status:** Current
**Last modified:** 2026-06-21 21:33 EDT

Installation paths for each surface of chatter. Pick the row that
matches what you want to do and the audience you belong to.

| If you want to... | Use this surface | Start here |
|---|---|---|
| Validate, normalize, convert, or batch-process CHAT files | `chatter` CLI | [CLI installation](../chatter/user-guide/installation.md) |
| Embed the Rust crates in another program | Rust libraries | [Library usage](../chatter/integrating/library-usage.md) |
| Reuse the grammar in editor or parser tooling | `tree-sitter-talkbank` | crate docs plus the [CHAT format overview](../chat-format/overview.md) |

`chatter` is publicly released: the CLI and desktop app are available from the
[latest GitHub release](https://github.com/TalkBank/chatter/releases/latest).
The Rust crates and grammar are source-available from this repository (not yet
published to crates.io). As a 0.x release, APIs and flags may change before 1.0.

For audio + ML pipelines (transcribe, force-align, morphotag,
benchmark), see the upstream `batchalign3` project, that lives
outside the chatter repo and has its own installation flow.
