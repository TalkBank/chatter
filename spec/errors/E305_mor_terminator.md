+++
code = 'E305'
name = 'Morphology requires its own terminator'

[[example]]
title = 'Terminated lexical morphology'
level = 'tier'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	Mommy will get them .
%mor:	propn|Mommy aux|will verb|get-Inf pron|they-Prs-Acc-Plur .
@End
'''

[[example]]
title = 'Delete only the morphology terminator'
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	Mommy will get them .
%mor:	propn|Mommy aux|will verb|get-Inf pron|they-Prs-Acc-Plur 
@End
'''
+++

## Description

This pair reduces the second lexical turn of
`corpus/reference/tiers/mor-gra.cha`, without its grammatical-relations tier.
The invalid example deletes only the morphology terminator, retaining the
complete main tier. The grammar permits this omission for diagnosis, but the
parser rejects the incomplete morphology with E305 rather than inventing a
terminator from the main tier.

## CHAT Rule

A lexical `%mor` tier requires its own terminator. A terminated main tier does
not make an unterminated morphology tier valid. Any proposed terminator remains
a user choice, and the changed transcript must still satisfy cross-tier rules.

## Notes

The parser rejects this morphology tier and taints its utterance even though
the CST needs no recovery nodes. The fix catalog can locate its terminal
newline, but that alone does not authorize writing: edit admission refuses
all three proposals as `TaintedUtterance`. The canonical catalog regression
checks this refusal for LF and CRLF without bypassing admission.
