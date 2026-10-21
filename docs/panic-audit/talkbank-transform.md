# Panic audit: talkbank-transform

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/panic-audit/talkbank-transform.md)

See [shared policy](README.md) for enforcement and verification limits.
The package inherits the workspace restriction lints.

[num_words/num2text.rs](../../crates/talkbank-transform/src/num_words/num2text.rs)
contains a production `unwrap_used` allowance on `NUM2LANG`.
Its `LazyLock` parses embedded `num2lang.json` when first accessed.
The source comment identifies compile-time-embedded data as the rationale;
embedding bytes does not itself establish that JSON parsing succeeds.

Keep data validity and execution of initialization in the verification scope
when changing this owner. The former zero-production-exception claim is not
true for this source. Test-only allowances elsewhere remain separate.
This guide does not certify all transformation paths, optional feature graphs
or dependencies.
