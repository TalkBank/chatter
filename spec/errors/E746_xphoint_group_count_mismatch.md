# E746: Xphoint group count does not match the pho word count

## Description

`%xphoint` must have exactly one ' / '-separated group per `%pho` word.

## Metadata

- **Error Code**: E746
- **Category**: Phon phone interval
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E746

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat dog . 0_500
%pho:	kæt dɒɡ
%xphoint:	k 0_100 æ 100_200 t 200_300
@Comment:	ERROR: %xphoint has 1 group but %pho has 2 words
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E746

## CHAT Rule

`%xphoint` must have exactly one ' / '-separated group per `%pho` word.
