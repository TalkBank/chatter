# E739: Phoaln pair is malformed

## Description

Every `%xphoaln` pair has exactly one ↔ with a non-null phone on at least one side.

## Metadata

- **Error Code**: E739
- **Category**: Phon phone alignment
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E739

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat .
%mod:	kæt
%pho:	kæt
%xphoaln:	∅↔∅,k↔k,æ↔æ,t↔t
@Comment:	ERROR: the '∅↔∅' pair is never legal (both sides null)
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E739

## CHAT Rule

Every `%xphoaln` pair has exactly one ↔ with a non-null phone on at least one side.
