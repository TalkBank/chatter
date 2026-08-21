+++
code = 'E730'
name = 'Bullet timing gap'
kind = 'Invalidity'
status = 'not_implemented'

[[example]]
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello . 1000_2000
*MOT:	hi . 10000_11000
@End
'''
+++

**Status:** Not implemented, reserved
**Last updated:** 2026-07-31

## Description

The gap between the current tier's bullet BEG time and the previous tier's
bullet END time exceeds an acceptable discontinuity threshold. Intended to
be reported only in bullet consistency mode (CLAN `+c0`).

**Validation not yet implemented for this spec example.** No production
code path constructs this `ErrorCode` variant, and no bullet-consistency
CLI mode (`+c0`/`+c1`-equivalent) exists in chatter today. The only
references in the workspace before this spec were the CHECK-parity number
mapping (`crates/talkbank-parser-tests/src/check_error_map.rs`) and the
enum variant's own doc comment
(`crates/talkbank-model/src/errors/codes/error_code.rs`); there is no
reserved constant for it in `temporal.rs` (unlike E729/E731).

## Expected Behavior

Once implemented, and only under bullet consistency mode, validation
should report E730 on `*MOT`'s utterance for the discontinuity between
`*CHI`'s bullet END (2000ms) and `*MOT`'s bullet BEG (10000ms). This spec
deliberately does not assert a specific threshold value: the enum variant's
doc comment describes an "acceptable discontinuity threshold" without
naming a number, and none has been implemented.

## CHAT Rule

Corresponds to CLAN CHECK error 85 ("Gap found between current BEG time and
previous' tier END time"). See CHAT manual on media bullets:
<https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

## Notes

- CHECK-parity mapping: CLAN error 85 ->
  `crates/talkbank-parser-tests/src/check_error_map.rs`.
- Gated on a bullet-consistency mode (CLAN `+c0`) that chatter does not
  currently expose via any CLI flag.
