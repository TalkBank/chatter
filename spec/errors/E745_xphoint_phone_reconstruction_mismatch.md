# E745: Xphoint group does not reproduce the pho word

## Description

Concatenating a `%xphoint` group's phones must reproduce the corresponding `%pho` word.

## Metadata

- **Error Code**: E745
- **Category**: Phon phone interval
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E745

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat . 0_200
%pho:	kæt
%xphoint:	k 0_100 æ 100_200
@Comment:	ERROR: the %xphoint group reconstructs to 'kæ', which does not match %pho 'kæt'
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E745

## CHAT Rule

Concatenating a `%xphoint` group's phones must reproduce the corresponding `%pho` word.
