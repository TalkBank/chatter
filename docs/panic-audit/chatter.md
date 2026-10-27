# Panic audit: chatter

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/panic-audit/chatter.md)

See [shared policy](README.md) for enforcement and verification limits.
The CLI package inherits the workspace restriction lints.

Command decoding and word-report serialization return typed errors. They no
longer rely on `expect` assertions about clap or derived serializers. Text and
JSON stdout writes propagate failure through `CommandResult`, so a closed pipe
returns status 1 without a printing panic. The program thread still propagates
an unexpected unwind to the main thread; a recoverable command error is never
implemented by unwinding.

Test-only module allowances are separate. These ownership changes are not an
exhaustive panic-safety certificate for the CLI or its dependencies.

Use the maintained verification workflow rather than assuming a normal build
runs the restriction lints.
