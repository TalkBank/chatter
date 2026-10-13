+++
code = 'E203'
name = 'POS and word-internal features do not license two form suffixes'

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	chirrup@o@b$n .
@End
'''

[[example]]
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	chirrup@o$n .
@End
'''

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	chir^rup@o@b .
@End
'''

[[example]]
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	chir^rup@o .
@End
'''

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	chirrup:@o@b .
@End
'''

[[example]]
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	chirrup:@o .
@End
'''

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	tʃi@u@b$n .
@End
'''

[[example]]
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
*SPK:	tʃi@u$n .
@End
'''

+++

## Description

A word may carry at most one @ suffix under the existing maintainer ruling. A POS tag, syllable pause, lengthening or phonetic body does not make a second suffix legal. Each invalid case adds @b immediately after the existing form suffix and before any POS tag. The parser reports E203 while retaining the repeated marker in its observed model; it is not a valid phonetic @u form, and retaining a model does not establish error-free parsing.

## CHAT Rule

These controls extend the existing E203 policy to independently authored word-feature combinations. The paired examples change only the described word or annotation; other source bytes and file options remain fixed.
