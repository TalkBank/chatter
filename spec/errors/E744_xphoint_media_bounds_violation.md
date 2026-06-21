# E744: Xphoint intervals fall outside the media bullet

## Description

The first start and last end of `%xphoint` must lie within the `*SPK:` media bullet (1 ms tolerance).

## Metadata

- **Error Code**: E744
- **Category**: Phon phone interval
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E744

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat . 0_300
%pho:	kæt
%xphoint:	k 0_100 æ 100_200 t 200_400
@Comment:	ERROR: the last interval end 400 is outside the record media bullet 0-300
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E744

## CHAT Rule

The first start and last end of `%xphoint` must lie within the `*SPK:` media bullet (1 ms tolerance).
