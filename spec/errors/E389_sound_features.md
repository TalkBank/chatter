+++
code = 'E389'
name = 'Filler features do not license replacement'

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
*SPK:	&-hmm: [: pause] .
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
*SPK:	&-hmm: .
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
*SPK:	&-h:um [: pause] .
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
*SPK:	&-h:um .
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
*SPK:	&-erm@b [: pause] .
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
*SPK:	&-erm@b .
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
*SPK:	⌈anchor &-uh⌉ [: pause] .
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
*SPK:	⌈anchor &-uh⌉ .
@End
'''

+++

## Description

Lengthening, a form suffix and an attached overlap boundary preserve the filler category. E389 forbids replacement on that filler, not on the surrounding ordinary word. Each invalid case adds only one replacement annotation; the overlap pair remains balanced.

## CHAT Rule

These controls extend the existing E389 policy to independently authored word-feature combinations. The paired examples change only the described word or annotation; other source bytes and file options remain fixed.
