# Core generator agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/tools/AGENTS.md)

Read [spec guidance](../AGENTS.md) and the
[repository guidance](../../AGENTS.md) first. The contributor
[Spec System](../../book/src/architecture/spec-system.md) and
[Spec Workflow](../../book/src/contributing/spec-workflow.md) own the format
and commands; this file describes implementation boundaries.

## Workspace and dependencies

The `generators` package belongs to the independent `spec/Cargo.toml` workspace,
alongside `runtime-tools` and `vocabulary`.

Keep core generators independent of the live `talkbank-parser` and
`talkbank-model` crates. Tree-sitter and the CHAT grammar are permitted here
for wrapping examples and deriving CSTs. Work requiring the live model,
`ErrorCode` enum, validation or public parser belongs in `spec/runtime-tools`.
Shared authored schemas and diagnostic vocabulary belong in `spec/vocabulary`;
do not copy their metadata into either consumer.

This workspace has its own lint policy in `spec/Cargo.toml`. Do not infer that
the root workspace's lint settings are inherited, or change the policy merely
to make a check pass. Use typed input admission and explicit errors at authoring
and filesystem boundaries; build-time failure is not permission to fabricate
fallback values.

## Producer ownership

- `src/spec/` loads authored inputs against the shared schemas.
- `src/output/` renders corpus, Rust-test and documentation artifacts.
- `src/artifacts.rs` owns core artifact destinations and builders; runtime
  artifacts are owned by the sibling runtime crate.
- `src/generated/` contains generated vocabulary sites, not authored lists.
- `templates/` contains the Tera files; `src/templates/` is their Rust tooling.

Artifacts return their intended files before the owning output layer writes
them. Preserve the distinction between named-file ownership in a shared
directory and exclusive generated-directory ownership. Retain byte-identical
outputs; prune only what that owner is entitled to remove. Never replace
checked ownership with an arbitrary recursive delete or silently follow
linked entries. Preserve output-planning and partial-failure semantics.

Changes to a producer must carry its generated consumers and currency contract.
Read the current artifact registry and its generated book table rather than
adding another list of binaries or destinations to these instructions.
The obsolete authored `layer` field is not restored: stages are observed in
the generated snapshot and claims are adjudicated separately.

## Commands and evidence

From the repository root, the package-specific test command is:

```sh
cargo test --manifest-path spec/Cargo.toml -p generators
```

Use it only when relevant to the implementation change. The broader
spec-workspace test command and regeneration sequence live in Spec Workflow;
`just spec-gen` and `just spec-check` cover both core and runtime artifacts.
A bare `cargo test` at the repository root tests a different workspace.

Keep native boundary tests for ownership refusal, missing inputs, wire formats
and output behavior that the types alone cannot prove. Mutation candidates
are unreviewed data, never diagnostic oracles; preserve seed provenance and
review promotion through the normal spec workflow.

Documentation-only edits do not require rebuilding these crates. Local
generator tests do not establish independent public bootstrap, whole-language
completeness or release readiness.
