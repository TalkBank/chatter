+++
code = 'E746'
name = 'Xphoint group count does not match the pho word count'
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
*CHI:	cat dog . \u00150_500\u0015
%pho:	kæt dɒɡ
%xphoint:	k \u00150_100\u0015 æ \u0015100_200\u0015 t \u0015200_300\u0015
@Comment:	ERROR: %xphoint has 1 group but %pho has 2 words
@End
"""
+++

## Description

`%xphoint` must have exactly one ' / '-separated group per `%pho` word.

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E746

## CHAT Rule

`%xphoint` must have exactly one ' / '-separated group per `%pho` word.
