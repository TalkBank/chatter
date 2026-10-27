# Panic audit: talkbank-parser-re2c

**Status:** Reference
**Last updated:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/panic-audit/talkbank-parser-re2c.md)

See [shared policy](README.md) for enforcement and verification limits.
The package inherits workspace lints but also has module-level allowances.

## Generated lexer

The [lexer module](../../crates/talkbank-parser-re2c/src/lib.rs) allows
`panic`, `unwrap_used` and `expect_used` around the included DFA.
Its comment describes an internal-error terminal state as unreachable by
construction; that rationale is not by itself proof over all lexer inputs.

Ordinary builds copy the committed vendored lexer into `OUT_DIR`; they do
not regenerate it. [build.rs](../../crates/talkbank-parser-re2c/build.rs)
owns that copy and documents explicit pinned-tool regeneration. Review the
generator and generated artifact together when changing this boundary.

## Other allowances

Conversion modules `headers.rs`, `items.rs`, `text_tiers.rs` and
`tiers.rs` allow `unreachable`, `unwrap_used` and `expect_used`;
`words.rs` allows `expect_used`. Inspect these broad permissions with
their actual calls. They do not establish that every permitted panic is
necessary or justified. Historical retrace/annotation examples are not a
current exception inventory.

Fixture loading lives only in the integration target's `fixture_utils` module.
Its nonempty fixture type refuses unavailable or empty files before exposing
entries, and the requesting test reports that refusal as a failure. The former
public `tests_support` module and its production panic allowance are removed.

Build-script allowances and test-only attributes are separate from runtime
parser guarantees. This guide supplies no new lint or panic-freedom receipt.
