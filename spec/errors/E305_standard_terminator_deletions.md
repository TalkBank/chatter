+++
code = 'E305'
name = 'Standard terminator deletion preserves neighboring turns'

[[example]]
title = "Admitted standard-terminator reference"
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|3;00.||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Standard terminators: period (.), question (?), exclamation (!)
@Comment:	Constructs: period, question, exclamation
*CHI:	I see a cat .
*MOT:	where is the cat ?
*CHI:	wow it is so big !
*MOT:	yes it is a big cat .
@End
'''

[[example]]
title = "Delete only turn 1's . terminator"
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|3;00.||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Standard terminators: period (.), question (?), exclamation (!)
@Comment:	Constructs: period, question, exclamation
*CHI:	I see a cat 
*MOT:	where is the cat ?
*CHI:	wow it is so big !
*MOT:	yes it is a big cat .
@End
'''

[[example]]
title = "Delete only turn 2's ? terminator"
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|3;00.||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Standard terminators: period (.), question (?), exclamation (!)
@Comment:	Constructs: period, question, exclamation
*CHI:	I see a cat .
*MOT:	where is the cat 
*CHI:	wow it is so big !
*MOT:	yes it is a big cat .
@End
'''

[[example]]
title = "Delete only turn 3's ! terminator"
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|3;00.||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Standard terminators: period (.), question (?), exclamation (!)
@Comment:	Constructs: period, question, exclamation
*CHI:	I see a cat .
*MOT:	where is the cat ?
*CHI:	wow it is so big 
*MOT:	yes it is a big cat .
@End
'''

[[example]]
title = "Delete only turn 4's . terminator"
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|3;00.||||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Standard terminators: period (.), question (?), exclamation (!)
@Comment:	Constructs: period, question, exclamation
*CHI:	I see a cat .
*MOT:	where is the cat ?
*CHI:	wow it is so big !
*MOT:	yes it is a big cat 
@End
'''

+++

## Description

Each invalid example deletes exactly one complete main-tier terminator from
`corpus/reference/content/terminators-standard.cha`, using the source-bound
admitted-seed mutation producer. The seed remains as the legal control. The
four deletions cover first, interior and final turns, and period, question and
exclamation forms; all other source bytes, including the preceding space and
terminal newline, are retained.

## CHAT Rule

Outside CA mode, each main tier requires its own terminator. A neighboring
speaker's terminated turn does not complete the unterminated turn. E305
reports the absent terminator while the parser retains the turn's typed
content. See [E305](E305.md) for the rule and CA exception.

## Notes

The producer emits unreviewed candidates, not diagnostic expectations. These
claims are independently justified by E305. A runtime contract regenerates the
four deletions and compares them with these promoted fixtures; restoring the
removed token must reproduce the seed. This does not authorize other mutation
families or establish exact CHECK diagnostic behavior.
