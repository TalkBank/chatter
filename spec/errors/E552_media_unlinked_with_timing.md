# E552: `@Media` declares `unlinked` but transcript carries timing

## Description

The `@Media` header's `unlinked` status declares that the transcript is
not time-aligned to the media file. Timing evidence anywhere in the
transcript contradicts that declaration: either the transcript really is
aligned (so `unlinked` must be removed), or the timing tier is stale (so
it must be removed). This is the inverse of E544 (declared linkage
without timing evidence).

Two timing surfaces are checked, and the diagnostic names which one
fired, because they demand different advice:

- **Main-tier bullets** (visible in CLAN as bullet marks): the media is
  in fact linked; the fix is removing `unlinked`. This is exactly CLAN
  CHECK error 124.
- **`%wor` word-level timing only** (invisible control characters inside
  the dependent tier): the transcript may be aligned, or the `%wor` tier
  may be an erroneous leftover; the message offers both remedies. Real
  CLAN CHECK does NOT fire 124 on this case (grounded empirically
  2026-07-07); flagging it is deliberate chatter-stricter modernization,
  so the message must carry the full explanation itself.

## Metadata

- **Status**: implemented
- **Status note**: `check_media_unlinked_has_no_timing` in
  `crates/talkbank-model/src/model/file/chat_file/validate/checks.rs`,
  sharing the collected main-tier bullets with E544's check. NOTE: the
  `%wor`-only surface is detected via `utt.alignments`, so it requires
  alignment processing (the CLI default); `--skip-alignment` skips it.
  Message-quality regression tests:
  `crates/talkbank-transform/tests/e552_message_quality.rs`; CLAN-parity
  grounding for the main-bullet case:
  `check_parity/fixtures/CHECK_124_media_unlinked_with_bullet.cha`.
- **Last updated**: 2026-07-07 14:09 EDT

- **Error Code**: E552
- **Category**: header_validation
- **Level**: file
- **Layer**: validation

## Example 1

**Trigger**: `@Media` declares `unlinked` but a main-tier utterance
carries a timing bullet (CLAN CHECK 124's case).

**Expected Error Codes**: E552

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	session-01, audio, unlinked
*CHI:	hello world .0_1500
@End
```

## Example 2

**Trigger**: `@Media` declares `unlinked`; no main-tier bullets, but the
`%wor` tier carries word-level timing bullets. Chatter-stricter than
CLAN (which accepts this); the message names the `%wor` tier and offers
both remedies.

**Expected Error Codes**: E552

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	session-01, audio, unlinked
*CHI:	hello .
%wor:	hello 0_500 .
@End
```

## Counter-examples (documentation only, do not fire E552)

- `@Media` with `unlinked` and NO timing anywhere: the correct use of
  `unlinked`; validates clean.
- `@Media` with no status and main-tier bullets: linked media with
  timing, the ordinary fully-linked state.

## Expected Behavior

One E552 per offending file, located at the `@Media` header span. The
message depends on the evidence surface (see Description). Severity:
error.

## Review history

- 2026-07-07: spec added retroactively (the check shipped in v0.2.0
  without one, against the every-code-has-a-spec policy) alongside the
  message split, prompted by a field report where the generic message
  blamed invisible `%wor` bullets and advised the wrong fix.

## CHAT Rule

The `@Media` header's optional third field declares linkage status;
`unlinked` promises an un-aligned transcript. See the CHAT manual's
Media header section.
