# E732: Missing bullet in bullet consistency mode

**Status:** Not implemented, reserved
**Last updated:** 2026-07-31

## Description

When bullet consistency mode is active (CLAN `+c0` or `+c1`), every main
tier is required to carry a media bullet. This code is intended to be
reported on a main tier that has none while that mode is active.

**Validation not yet implemented for this spec example.** No production
code path constructs this `ErrorCode` variant, and no bullet-consistency
CLI mode (`+c0`/`+c1`-equivalent) exists in chatter today. The only
references in the workspace before this spec were the CHECK-parity number
mapping (`crates/talkbank-parser-tests/src/check_error_map.rs`) and the
enum variant's own doc comment
(`crates/talkbank-model/src/errors/codes/error_code.rs`); there is no
reserved constant for it in `temporal.rs` (unlike E729/E731).

## Metadata

- **Status**: not_implemented

- **Error Code**: E732
- **Category**: Temporal validation
- **Level**: tier
- **Layer**: validation
- **Kind**: Invalidity

## Example 1

**Trigger**: A main tier with no bullet while bullet consistency mode is
active
**Expected Error Codes**: E732

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello . 1000_2000
*CHI:	world .
@End
```

## Expected Behavior

Once implemented, and only when bullet consistency mode is requested,
validation should report E732 on the second `*CHI:` utterance for lacking
a bullet. Outside that mode, an unbulleted tier is ordinary valid CHAT and
this spec's example does not, by itself, imply any diagnostic today.

## CHAT Rule

Corresponds to CLAN CHECK error 110 ("No bullet found on this tier"). See
CHAT manual on media bullets:
<https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

## Notes

- CHECK-parity mapping: CLAN error 110 ->
  `crates/talkbank-parser-tests/src/check_error_map.rs`.
- CLAN CHECK error 110 is also mapped to the already-implemented `E360`
  (per the CHECK-parity audit); E732 is a reserved, unimplemented duplicate
  specifically for the `+c0`/`+c1` bullet-consistency-mode case.
- Gated on a bullet-consistency mode that chatter does not currently expose
  via any CLI flag.
