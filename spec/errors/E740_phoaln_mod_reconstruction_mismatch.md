# E740: Phoaln model side does not reproduce the mod word

## Description

Concatenating the model (left) sides of `%xphoaln`, skipping ∅, must reproduce the `%mod` word.

## Metadata

- **Error Code**: E740
- **Category**: Phon phone alignment
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E740

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat .
%mod:	kæt
%pho:	kæ
%xphoaln:	k↔k,æ↔æ
@Comment:	ERROR: the %xphoaln model side concatenates to 'kæ', which does not match %mod 'kæt'
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E740

## CHAT Rule

Concatenating the model (left) sides of `%xphoaln`, skipping ∅, must reproduce the `%mod` word.
