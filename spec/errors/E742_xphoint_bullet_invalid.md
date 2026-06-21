# E742: Xphoint bullet has start >= end

## Description

Each `%xphoint` phone interval must have start strictly less than end.

## Metadata

- **Error Code**: E742
- **Category**: Phon phone interval
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E742

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat . 0_300
%pho:	kæt
%xphoint:	k 0_100 æ 100_200 t 250_200
@Comment:	ERROR: the bullet for 't' has start 250 >= end 200
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E742

## CHAT Rule

Each `%xphoint` phone interval must have start strictly less than end.
