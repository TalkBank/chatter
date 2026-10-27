+++
code = 'E316'
name = 'Unparsable content'

[[example]]
title = 'Clitic marker without lexical material'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	~ .
@End
'''

[[example]]
title = 'Fragment prefix and clitic marker without lexical material'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	&+~ .
@End
'''

[[example]]
title = 'Pitch marker without lexical material'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	↑ .
@End
'''

[[example]]
title = 'CA delimiters and pitch markers without lexical material'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	°↑ .
@End
'''
+++

**Status:** Current
**Last updated:** {{git-dates:page}}

## Description

Word-internal markers do not supply lexical material. These examples lack the
text, shortening or stress entry required by `word_body`. A prefix alone does
not turn the following markers into a word.

## Expected Behavior

Reject the malformed content with an error. The native parser must not assemble
these standalone tokens into lexical words.

## CHAT Rule

The canonical `word_body` grammar requires a lexical entry after an initial
marker sequence. Standalone zero actions and overlap points have their own
productions; they do not relax word admission.
