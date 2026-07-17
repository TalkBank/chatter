# E738: Phosyl does not reproduce the pho word

## Description

Stripping `:CODE` from each `%xphosyl` unit must reproduce the
corresponding `%pho` word. A pause filler (`(.)`, `(..)`, `(...)`) on
`%xphosyl` must mirror the same pause token as the `%pho` word at that
position.

## Metadata

- **Error Code**: E738
- **Category**: Phon syllabification content
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Expected Error Codes**: E738

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat .
%pho:	kæt
%xphosyl:	k:Oæ:N
@Comment:	ERROR: stripping codes from %xphosyl gives 'kæ', which does not match %pho 'kæt'
@End
```

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E738

## CHAT Rule

Stripping `:CODE` from each `%xphosyl` unit must reproduce the
corresponding `%pho` word. A pause filler (`(.)`, `(..)`, `(...)`) on
`%xphosyl` must mirror the same pause token as the `%pho` word at that
position.
