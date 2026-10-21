# Panic audit: chatter

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/panic-audit/chatter.md)

See [shared policy](README.md) for enforcement and verification limits.
The CLI package inherits the workspace restriction lints.

Selected production `expect_used` owners are:

- [commands/clean.rs](../../crates/chatter/src/commands/clean.rs):
  JSON serialization of derived clean-entry output. The inline rationale
  depends on that type and its serializers.
- [main.rs](../../crates/chatter/src/main.rs): conversion of Clap matches
  into the CLI type after command parsing. The inline rationale depends on
  the command definition and typed decoder agreeing.

Review those dependencies when changing either boundary; prior validation
is not an unconditional proof about future serializers or CLI definitions.
Test-only module allowances are separate. This list replaces stale file-routing
examples but is not an exhaustive panic-safety certificate.

Use the maintained verification workflow rather than assuming a normal build
runs the restriction lints.
