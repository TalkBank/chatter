+++
code = 'E209'
name = 'Markers do not combine adjacent shortenings into one CA omission'

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
*SPK:	⌈(ma)(ybe) (soon)⌉ .
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
*SPK:	⌈(maybe) (soon)⌉ .
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
*SPK:	⌈(la)(ter)⌉ .
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
*SPK:	⌈(later)⌉ .
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
*SPK:	∆(ne)(ar) (far)∆ .
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
*SPK:	∆(near) (far)∆ .
@End
'''

+++

## Description

Under CA mode exactly one shortening with nonlexical markers normalizes to a CA omission. Splitting it into two adjacent shortenings does not. Overlap and paired prosodic markers are preserved on both sides of each control; they cannot supply the missing spoken material. This extends the existing single-versus-two-shortening rule without changing normalization.

## CHAT Rule

These controls extend the existing E209 policy to independently authored word-feature combinations. The paired examples change only the described word or annotation; other source bytes and file options remain fixed.
