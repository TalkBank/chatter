+++
code = 'E731'
name = 'Speaker bullet self-overlap via timing'
kind = 'Invalidity'
status = 'not_implemented'

[[example]]
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello . 1000_2000
*CHI:	world . 1500_2500
@End
'''
+++

**Status:** Not implemented, reserved
**Last updated:** 2026-07-31

## Description

A speaker's bullet start time (BEG) is before their own previous bullet's
end time (END), checked purely from bullet timing rather than from overlap
markers. This is intended to supplement E704 (`SpeakerSelfOverlap`, which
checks overlap markers `⌈⌉`/`⌊⌋`) with an actual-timing check for the same
condition, without E704's 500ms tolerance.

**Validation not yet implemented for this spec example.** No production
code path constructs this `ErrorCode` variant; the only references in the
workspace before this spec were the CHECK-parity number mapping
(`crates/talkbank-parser-tests/src/check_error_map.rs`) and the reserved
constant `crates/talkbank-model/src/errors/codes/temporal.rs::E731`. The
bullet-timing self-overlap check itself does not exist.

## Expected Behavior

Once implemented, validation should report E731 on the second `*CHI:`
utterance: its bullet BEG (1500ms) is before the first utterance's bullet
END (2000ms), with no tolerance applied (unlike E704's 500ms allowance).

## CHAT Rule

Corresponds to CLAN CHECK error 133 ("BEG time is smaller than same
speaker's previous END time"). See CHAT manual on media bullets:
<https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

## Notes

- Reserved constant: `crates/talkbank-model/src/errors/codes/temporal.rs`.
- CHECK-parity mapping: CLAN error 133 ->
  `crates/talkbank-parser-tests/src/check_error_map.rs`.
- Distinct from E704, which checks overlap markers (`⌈⌉`/`⌊⌋`) rather than
  bullet timing, and allows a 500ms tolerance this check would not.
