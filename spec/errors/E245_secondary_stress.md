+++
code = 'E245'
name = 'Secondary stress needs spoken material'

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
*SPK:	ˈˌ .
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
*SPK:	ˈriˌver .
@End
'''

+++

## Description

The existing E245 rule applies to secondary as well as primary stress. The legal control has both stress levels, satisfying E250. Removing all spoken segments preserves those markers but leaves a marker-only word, rejected during parsing before a fabricated zero-content Word can enter validation. Recovery is not a valid or byte-exact control.

## CHAT Rule

These controls extend the existing E245 policy to independently authored word-feature combinations. The paired examples change only the described word or annotation; other source bytes and file options remain fixed.
