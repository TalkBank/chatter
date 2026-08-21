+++
code = 'E743'
name = 'Xphoint interval starts are not non-decreasing'
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
*CHI:	cat . \u00150_300\u0015
%pho:	kæt
%xphoint:	k \u00150_100\u0015 æ \u0015200_300\u0015 t \u001550_150\u0015
@Comment:	ERROR: the third interval start 50 is before the previous start 200
@End
"""
+++

## Description

`%xphoint` interval start times must be non-decreasing across the tier.

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E743

## CHAT Rule

`%xphoint` interval start times must be non-decreasing across the tier.
