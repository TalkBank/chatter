# CHECK Parity Audit

**Status:** Current
**Last updated:** 2026-09-05

`chatter validate` is the binding CHAT validator, but a rejection must first be
adjudicated: parser/validator defect or invalid data? Assume a Chatter defect
until the specification and source evidence establish otherwise. Do not edit
corpus data merely to make a diagnostic disappear. CHECK remains a useful
independent source of counterexamples, not a substitute for that adjudication.

## Three different kinds of evidence

1. **Code mapping.** The generated inventory at
   `docs/audits/check-parity-audit.md` joins the committed CHECK reference with
   Chatter's compiled error-code registry. A curated mapping identifies related
   checks; it proves neither semantic completeness nor runtime agreement.
   Missing mappings likewise do not prove missing validation.
2. **Adjudicated expectations.**
   `crates/talkbank-parser-tests/tests/check_parity/manifest.json` records fixture
   expectations, intentional divergences and cases with no file-mode obligation.
   `just spec-status` summarizes these declarations and verifies spec examples.
3. **Executed behavior.** `chatter_matches_check` tests Chatter against the
   manifest. The ignored `clan_check_grounding` test runs the actual CHECK
   executable through `CHATTER_CLAN_RUN`, a file-mode PTY wrapper. Running this
   test explicitly without the wrapper is an error, not a passing skipped audit.

## Regenerating the mapping inventory

```bash
cargo run -p talkbank-parser-tests --bin audit_check_parity
```

The library module `check_mapping_audit` owns the report. Its mapping type has
only unmapped and nonempty curated states, with no runtime-parity certificate.
The error-code registry comes from `ErrorCode::iter()`, generated from
`spec/codes/error-codes.toml`; source-file moves cannot erase its inventory.
The supplementary ID table and `check_error_map` both use compiled
`ErrorCode` variants, so retired or renamed codes cannot silently disappear.
No message-keyword fallback is used. The integration gate checks
that the committed report matches a fresh render.

For runtime checks, use the commands in
[Spec Workflow](../../contributing/spec-workflow.md). Record the source and
executable revision when refreshing CHECK evidence. The committed mapping
report deliberately carries no verified behavioral-parity count.

## Triaging a gap

A CHECK rule with no TalkBank mapping is **not** automatically a `chatter` bug.
Each gap is triaged against the CLAN source (`OSX-CLAN/src/clan/check.cpp`) into
one of four buckets:

- **(a) Genuine gap.** CHECK enforces a real CHAT rule `chatter` is missing.
  *Action: implement it in `chatter`* through a spec example and its generated fixture, then run the
  focused validator and parity gates. Update curated mapping evidence if needed. Example: curly single quotes (see below).
- **(b) Intentional divergence.** CHECK's active rule is wrong or a
  text-hack `chatter` deliberately does not reproduce. *Action: document the
  divergence, do not implement.* CHECK error 109 (postcodes on dependent tiers)
  is the worked example below.
- **(c) No obligation in file mode.** A retired, GUI-only or unreachable CHECK
  path needs a typed reason in the manifest. CHECK 49 is commented out; it is
  not an active rule from which Chatter intentionally diverges.
- **(d) Additional Chatter validation.** Establish the actual enforced rule
  before describing an unmapped Chatter code as an enhancement. A code can
  instead be dormant, deprecated or awaiting mapping.

An unmapped row alone does not establish priority or release readiness. The
manifest adjudication and supported-input contract determine its obligation.

## Worked example: E256 (CHECK 138/139), implemented across both parsers

Curly single quotes (`U+2018`, `U+2019`) used as word characters were a genuine
gap (bucket a): CHECK errors 138/139 flag them, `chatter` previously absorbed
them silently. They are illegal CHAT word characters; CHAT uses the ASCII
apostrophe.

Because `chatter` has two parsers that must agree (the tree-sitter parser and
the re2c oracle, see [Parser Backends](../parser-backends.md)), the fix lands in
both, reaching the same recovery:

- The character is **excluded from the word token** via the shared
  [Symbol Registry](../symbol-registry.md) (so it can never be part of a word).
- The **tree-sitter grammar** recognizes it as a dedicated `illegal_curly_quote`
  node (not a generic parse error), and the parser emits `E256` with a span
  pointing at the exact character.
- The **re2c lexer** emits a recognized `IllegalCurlyQuote` token; the
  file-level parser emits `E256` and drops the token before parsing.
- In both, the offending quote is **dropped and the surrounding words survive**,
  so validation continues and reports a precise, actionable diagnostic.

This is the canonical shape of a CHECK-parity rule implemented to `chatter`'s
standards: a recognized construct (parse, don't merely fail), the same behavior
in both parsers, and a spec in `spec/errors/` that drives the tests.

## Worked example: CHECK 109 (intentional divergence, do not implement)

CHECK error 109 ("Postcodes are not allowed on dependent tiers") is the canonical
bucket-(b) divergence. CLAN fires it from `check_CheckWords`
(`OSX-CLAN/src/clan/check.cpp:3471-3690`) whenever a `%`-tier word matches the raw
character pattern `[+ ` or `[- ` (the `isPostCodeMark` macro), on any non-`%x`
dependent tier. `chatter` deliberately does not reproduce it, for two reasons.

- **There is nothing typed to flag.** `chatter` models postcodes as structured
  `Postcode` nodes inside `TierContent.postcodes`, a slot carried by the main tier.
  Ordinary dependent tiers have their own tier types (`%com` is a text tier) with
  no postcode slot, so a `[+ ...]`-shaped token on one is just part of the tier
  text. Detecting it would require a raw character scan of the tier string, the
  banned CHAT text-hacking; there is no structured node to validate.
- **It is not a CHAT-validity rule.** An empirical check (2026-06-25) ran the real
  CLAN *analysis* tools on dependent-tier postcodes: FREQ and MLU exclude the
  `[+ ...]` token from their counts exactly as they do on the main tier, and KWAL
  prints the line without error. No analysis tool chokes; only CHECK flags it, so
  CHECK 109 guards against a failure mode its own toolchain does not have.

The divergence is grounded permanently as a `divergence` entry (CHECK 109) in the
behavioral parity manifest
(`crates/talkbank-parser-tests/tests/check_parity/manifest.json`): the
`chatter_matches_check` gate asserts `chatter` keeps validating the fixture clean
(a permanent intentional state, not a gap to close), and `clan_check_grounding`
re-confirms the real CLAN binary still emits 109.

## Related

- [Bullet Validation](../bullet-validation.md) documents the temporal media-bullet
  checks (CLAN errors 83/133/84 and `chatter`'s E701/E704/E729), a specific
  instance of the same "match CHECK where it is right, diverge where it is wrong"
  reconciliation this audit tracks across the whole error set.
- [Errors, CHAT core](chat-core-errors.md) describes the `ErrorCode` model and
  the parser-layer / validation-layer split.
- The spec-driven test pipeline that backs every rule is in
  [Testing](../../contributing/testing.md): rules live in `spec/errors/` and
  generate both parser tests and the validation corpus.
