+++
code = 'E745'
name = 'Xphoint group does not reproduce the pho word'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	cat . \u00150_200\u0015
%pho:	kæt
%xphoint:	k \u00150_100\u0015 æ \u0015100_200\u0015
@Comment:	ERROR: the %xphoint group reconstructs to 'kæ', which does not match %pho 'kæt'
@End
"""
+++

## Description

Concatenating a `%xphoint` group's phones must reproduce the corresponding `%pho` word.

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E745

## CHAT Rule

Concatenating a `%xphoint` group's phones must reproduce the corresponding `%pho` word.
