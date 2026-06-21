# E743: Xphoint interval starts are not non-decreasing

## Description

`%xphoint` interval start times must be non-decreasing across the tier.

## Metadata

- **Error Code**: E743
- **Category**: Phon phone interval
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E743

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat . 0_300
%pho:	kæt
%xphoint:	k 0_100 æ 200_300 t 50_150
@Comment:	ERROR: the third interval start 50 is before the previous start 200
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E743

## CHAT Rule

`%xphoint` interval start times must be non-decreasing across the tier.
