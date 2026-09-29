+++
code = 'E370'
name = 'Retraced material split across utterances'

[[example]]
title = 'Unsplit repetition control'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the dog> [/] the dog runs .
@End
'''

[[example]]
title = 'Split repetition with an exact successor prefix'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the dog> [/] .
*CHI:	the dog runs .
@End
'''

[[example]]
title = 'Split repetition with a different successor word'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the dog> [/] .
*CHI:	the cat runs .
@End
'''

[[example]]
title = 'Split repetition with an incomplete successor prefix'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the dog> [/] .
*CHI:	the .
@End
'''

[[example]]
title = 'Replacement target does not prove a plain-word repetition'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the dog> [/] .
*CHI:	a [: the] dog runs .
@End
'''

[[example]]
title = 'Unsplit correction control'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the dog> [//] a cat runs .
@End
'''

[[example]]
title = 'Split correction need not repeat the abandoned words'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the dog> [//] .
*CHI:	a cat runs .
@End
'''

[[example]]
title = 'An intervening header prevents automatic joining'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the dog> [/] .
@Comment:	A boundary that must remain before the following turn.
*CHI:	the dog runs .
@End
'''
[[example]]
title = 'Unsplit repetition chain control'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the [/] the [/] the dog .
@End
'''

[[example]]
title = 'Both boundaries in a repetition chain are split'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the [/] .
*CHI:	the [/] .
*CHI:	the dog .
@End
'''

[[example]]
title = 'Unsplit repetition containing a pause'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the (.) dog> [/] the dog runs .
@End
'''

[[example]]
title = 'Split repetition with non-lexical material needs broad repair opt-in'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<the (.) dog> [/] .
*CHI:	the dog runs .
@End
'''

[[example]]
title = 'Adjacent ordinary turns are not dangling retraces'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the dog .
*CHI:	the dog runs .
@End
'''
+++

**Status:** Current
**Last updated:** 2026-09-26 13:09 EDT

## Description

Authored controls and deliberate turn-split variants. Moving the speech after
a retrace marker to a separate utterance leaves E370 in the first utterance,
even when the next speaker is the same. These examples are not corpus-derived.

## CHAT Rule

Repeated or corrected material must follow its retrace marker within the same
utterance. A later utterance does not satisfy that requirement.

## Expected Behavior

Automatic joining is a separate, explicitly selected repair policy, not a
validity rule or a claim about the speaker's intent. The conservative policy
requires an exact plain-word successor prefix. Corrections require opt-in;
the broadest policy also permits nonmatching repetitions. No policy may move
speech across an intervening header. The unsplit repetition and correction
controls define the expected complete models for the corresponding joins.
Material containing a pause is not a plain-word repetition witness: only the
broadest repair scope may join its split form. Ordinary consecutive turns
without a dangling retrace must remain separate under every scope.
