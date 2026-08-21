+++
code = 'E737'
name = 'Modsyl does not reproduce the mod word'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
claim = 'violates'
chat = '''
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
'''
+++

## Description

Stripping `:CODE` from each `%xmodsyl` unit must reproduce the
corresponding `%mod` word. A pause filler (`(.)`, `(..)`, `(...)`) on
`%xmodsyl` must mirror the same pause token as the `%mod` word at that
position.

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E737

## CHAT Rule

Stripping `:CODE` from each `%xmodsyl` unit must reproduce the
corresponding `%mod` word. A pause filler (`(.)`, `(..)`, `(...)`) on
`%xmodsyl` must mirror the same pause token as the `%mod` word at that
position.
