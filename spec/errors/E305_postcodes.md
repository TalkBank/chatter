+++
code = 'E305'
name = 'Missing terminator before final postcodes'

[[example]]
title = 'Terminated turn with two final postcodes'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello . [+ bch] [+ foo]
@End
'''

[[example]]
title = 'Delete the terminator but retain both final postcodes'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello  [+ bch] [+ foo]
@End
'''
+++

## Description

This pair reduces the first turn of
`corpus/reference/content/postcodes-and-freecodes.cha`. The invalid example
deletes only the period. Postcodes do not supply an utterance terminator.

## CHAT Rule

Outside CA mode, a main tier requires a terminator before its final postcodes.
A repair must preserve the postcodes and their final-code position, rather
than insert a terminator after them. The choice of terminator remains the
author's decision.
