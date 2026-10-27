+++
code = 'E342'
name = 'Word suffixes require a word body after omission zero'

[[example]]
title = 'Standalone action with explanation'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	0 [= points] .
@End
'''

[[example]]
title = 'Omitted word with form suffix'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	0hello@f .
@End
'''

[[example]]
title = 'Omitted word with language suffix'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	0hello@s:eng world .
@End
'''

[[example]]
title = 'Omitted word with POS suffix'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	0hello$n .
@End
'''

[[example]]
title = 'Form suffix without a word body'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	0@f .
@End
'''

[[example]]
title = 'Language suffix without a word body'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	0@s:eng world .
@End
'''

[[example]]
title = 'POS suffix without a word body'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	0$n .
@End
'''
+++

## Description

A standalone zero is an action, and may carry a spaced scoped annotation.
An omission prefix followed by a word body instead forms a word, which may
carry a form, language or POS suffix. Without that body, treating a word
suffix as an action and discarding it loses authored source. The invalid
controls remove only the word body from their corresponding legal words.

The primary grammar records the absent body after the omission prefix as a
MISSING `word_segment`, so E342 owns these missing-syntax claims. The language
pair includes an unmarked following word to isolate it from the independent
whole-utterance language-marker rule (E255).
