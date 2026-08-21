+++
code = 'E739'
name = 'Phoaln pair is malformed'
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
%pho:	kæt
%xphoaln:	∅↔∅,k↔k,æ↔æ,t↔t
@Comment:	ERROR: the '∅↔∅' pair is never legal (both sides null)
@End
'''
+++

## Description

Every `%xphoaln` pair has exactly one ↔ with a non-null phone on at least one side.

## Expected Behavior

- **Parser**: Should succeed, syntax is valid
- **Validator**: Should report E739

## CHAT Rule

Every `%xphoaln` pair has exactly one ↔ with a non-null phone on at least one side.
