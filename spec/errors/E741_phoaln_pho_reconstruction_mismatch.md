# E741: Phoaln actual side does not reproduce the pho word

## Description

Concatenating the actual (right) sides of `%xphoaln`, skipping ∅, must
reproduce the `%pho` word. The comparison is segment-level: stress markers (`\u{02C8}`, `\u{02CC}`)
and syllable-boundary notation (Phon's `^`, IPA's `.`) in either string are
ignored, since the alignment pairs carry bare segments while the source
word may carry suprasegmental and boundary notation.

## Metadata

- **Error Code**: E741
- **Category**: Phon phone alignment
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E741

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat .
%mod:	kæ
%pho:	kæt
%xphoaln:	k↔k,æ↔æ
@Comment:	ERROR: the %xphoaln actual side concatenates to 'kæ', which does not match %pho 'kæt'
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E741

## CHAT Rule

Concatenating the actual (right) sides of `%xphoaln`, skipping ∅, must
reproduce the `%pho` word. The comparison is segment-level: stress markers (`\u{02C8}`, `\u{02CC}`)
and syllable-boundary notation (Phon's `^`, IPA's `.`) in either string are
ignored, since the alignment pairs carry bare segments while the source
word may carry suprasegmental and boundary notation.
