+++
code = 'E387'
name = 'Replacement on a fragment with word-internal features'

[[example]]
title = 'Replacement on a lengthened fragment'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	&+br: [: branch] .
@End
'''

[[example]]
title = 'Lengthened fragment without replacement'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	&+br: .
@End
'''

[[example]]
title = 'Replacement on a fragment with interior lengthening'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	&+k:ra [: craft] .
@End
'''

[[example]]
title = 'Fragment with interior lengthening without replacement'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	&+k:ra .
@End
'''

[[example]]
title = 'Replacement on a fragment with segment repetition'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	&+↫b-b↫ba [: bay] .
@End
'''

[[example]]
title = 'Fragment with segment repetition without replacement'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	&+↫b-b↫ba .
@End
'''

[[example]]
title = 'Replacement on a fragment with phonetic form'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	&+tʃ@u [: cheer] .
@End
'''

[[example]]
title = 'Phonetic fragment without replacement'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	&+tʃ@u .
@End
'''
+++

## Description

Lengthening, segment repetition and phonetic form preserve the `&+`
phonological-fragment category. They do not make the fragment a word eligible
for a replacement. Each invalid example adds only one replacement annotation
to its adjacent legal control.

## CHAT Rule

The existing E387 rule applies to the produced fragment regardless of its
word-internal features or form suffix. A fragment without a replacement is
legal; attaching `[: text]` to it is not. Plain-fragment controls remain in
`E387.md`.
