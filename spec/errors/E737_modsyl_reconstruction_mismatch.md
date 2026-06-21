# E737: Modsyl does not reproduce the mod word

## Description

Stripping `:CODE` from each `%xmodsyl` unit must reproduce the corresponding `%mod` word.

## Metadata

- **Error Code**: E737
- **Category**: Phon syllabification content
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E737

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat .
%mod:	kæt
%xmodsyl:	k:Oæ:N
@Comment:	ERROR: stripping codes from %xmodsyl gives 'kæ', which does not match %mod 'kæt'
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E737

## CHAT Rule

Stripping `:CODE` from each `%xmodsyl` unit must reproduce the corresponding `%mod` word.
