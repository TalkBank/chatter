# E735: Syllabification unit is not a phone:CODE pair

## Description

Every `%xmodsyl`/`%xphosyl` unit must be one phone, an ASCII ':', then one
constituent code.

**Pause-filler exemption.** Phon keeps word-aligned phonology tiers in
index lockstep with the main tier: when the main tier carries a pause, the
pause token (`(.)`, `(..)`, `(...)`) is mirrored at the same word position
on `%mod`, `%pho`, `%xmodsyl`, and `%xphosyl`. Such a filler is a valid
word on the syllabification tiers and is exempt from the phone:CODE unit
rule (it must instead mirror the same pause on the source tier; see
E737/E738). Timed pauses (`(1.5)`) are not accepted as fillers: they are
unattested on syllabification tiers in the wild corpora.

## Metadata

- **Error Code**: E735
- **Category**: Phon syllabification content
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E735

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat .
%mod:	kæt
%xmodsyl:	:Oæ:Nt:C
@Comment:	ERROR: the first %xmodsyl unit ':O' has an empty phone before the code
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E735

## CHAT Rule

Every `%xmodsyl`/`%xphosyl` unit must be one phone, an ASCII ':', then one
constituent code.

**Pause-filler exemption.** Phon keeps word-aligned phonology tiers in
index lockstep with the main tier: when the main tier carries a pause, the
pause token (`(.)`, `(..)`, `(...)`) is mirrored at the same word position
on `%mod`, `%pho`, `%xmodsyl`, and `%xphosyl`. Such a filler is a valid
word on the syllabification tiers and is exempt from the phone:CODE unit
rule (it must instead mirror the same pause on the source tier; see
E737/E738). Timed pauses (`(1.5)`) are not accepted as fillers: they are
unattested on syllabification tiers in the wild corpora.
