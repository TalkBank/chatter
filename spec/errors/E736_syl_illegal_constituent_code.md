# E736: Illegal syllable constituent code

## Description

Constituent codes on `%xmodsyl`/`%xphosyl` must be one of O N C L R E A D U.

## Metadata

- **Error Code**: E736
- **Category**: Phon syllabification content
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E736

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat .
%pho:	kæt
%xphosyl:	k:Oæ:Nt:Z
@Comment:	ERROR: 'Z' is not a legal constituent code (legal: O N C L R E A D U)
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E736

## CHAT Rule

Constituent codes on `%xmodsyl`/`%xphosyl` must be one of O N C L R E A D U.
