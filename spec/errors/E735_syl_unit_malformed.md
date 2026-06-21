# E735: Syllabification unit is not a phone:CODE pair

## Description

Every `%xmodsyl`/`%xphosyl` unit must be one phone, an ASCII ':', then one constituent code.

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

Every `%xmodsyl`/`%xphosyl` unit must be one phone, an ASCII ':', then one constituent code.
