+++
code = 'E316'
name = 'Registry-owned word boundaries'

[[example]]
title = 'Equals is not spoken word text'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
*CHI:	ca=fé .
@End
'''

[[example]]
title = 'Left arrow is not spoken word text'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
*CHI:	ca←fé .
@End
'''

[[example]]
title = 'Upper-left arrow is not spoken word text'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
*CHI:	ca↖fé .
@End
'''

[[example]]
title = 'Lower-left arrow is not spoken word text'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
*CHI:	ca↙fé .
@End
'''

[[example]]
title = 'Ordinary Unicode text remains legal'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
*CHI:	café .
@End
'''

[[example]]
title = 'Left arrow cannot extend an event'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
*CHI:	&=ca←fé .
@End
'''

[[example]]
title = 'Upper-left arrow cannot extend an event'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
*CHI:	&=ca↖fé .
@End
'''

[[example]]
title = 'Lower-left arrow cannot extend an event'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI|||||Target_Child|||
*CHI:	&=ca↙fé .
@End
'''
+++

## Description

The symbol registry excludes `=`, `←`, `↖` and `↙` from spoken word
segments. Embedding one between ordinary letters does not turn it into spoken
text. The three arrows are also excluded from event descriptions. These examples distinguish lexical rejection from accepting a clean word.

## Expected Behavior

The primary parser reports E316 for the excluded characters. Every backend must
report an error; recovery shape and exact diagnostic identity may differ. The
ordinary Unicode word is a legal control. The authored character classes in
`spec/symbols/symbol_registry.json` own this boundary; parser-local copies must
not broaden it.
