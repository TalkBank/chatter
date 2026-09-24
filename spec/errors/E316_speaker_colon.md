+++
code = 'E316'
name = 'Missing main-tier speaker colon'

[[example]]
title = 'Complete speaker delimiter control'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||female|||Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
title = 'Delete only the colon while retaining the required tab'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||female|||Target_Child|||
*CHI	hello .
@End
'''
+++

## Description

A main-tier speaker prefix requires `*CODE:` followed by a tab. This paired
mutation deletes only the colon; the tab, speaker and utterance are unchanged.

## Expected Behavior

The control parses without recovery. The mutation reports E316: tree-sitter
does not repair this input with a MISSING colon node. Consequently this is not
an E322 witness, and its invalidity must not be mistaken for evidence that a
particular generated recovery slot is reachable.

## CHAT Rule

See the CHAT manual's
[speaker-code syntax](https://talkbank.org/0info/manuals/CHAT.html#Utterance_Speaker_Code).
