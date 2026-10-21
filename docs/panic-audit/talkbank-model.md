# Panic audit: talkbank-model

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/panic-audit/talkbank-model.md)

See [shared policy](README.md) for enforcement and verification limits.
The package inherits the workspace restriction lints.

The former overlap-loop unwrap examples are no longer present:
[cross_utterance/mod.rs](../../crates/talkbank-model/src/validation/cross_utterance/mod.rs)
uses `let Some(index) = orphan.region.index else { continue; }` in both
orphan loops. The admitted index is available directly, without a later unwrap.
This local control-flow invariant does not prove that every model, validation
or alignment operation cannot panic.

The build script has separate allowances. Build-time generation, test-only
attributes, runtime operations and dependencies need distinct review. Do not
infer runtime panic freedom from a search limited to explicit allowances.
