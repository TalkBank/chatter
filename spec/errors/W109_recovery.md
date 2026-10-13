+++
code = 'W109'
name = 'Unicode filename normalization preserves unrelated recovery'

[[example]]
title = 'Canonical media spelling with unrelated main-tier recovery'
source = 'error_corpus/validation_errors/Schlüssel.cha'
level = 'header'
claim = 'legal'
notes = 'Reuse the canonical W109 control and insert one unmatched closing bracket in the main tier. Legal asserts absence of W109, not validity of the transcript.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	Schlüssel, audio, unlinked
*CHI:	hello ] .
@End
'''

[[example]]
title = 'A clean media token remains locally repairable during unrelated recovery'
source = 'error_corpus/validation_errors/Schlüssel.cha'
level = 'header'
claim = 'violates'
notes = 'Change only the media token to u plus combining diaeresis. The header itself is clean; its verified-token edit must not repair or discard the unrelated unmatched bracket.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	Schlüssel, audio, unlinked
*CHI:	hello ] .
@End
'''

[[example]]
title = 'Canonical filename within a malformed media carrier'
source = 'error_corpus/validation_errors/Schlüssel.cha'
level = 'header'
claim = 'legal'
notes = 'Insert a second comma after the filename. The malformed header is not made valid by a canonical filename.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	Schlüssel,, audio, unlinked
*CHI:	hello .
@End
'''

[[example]]
title = 'A malformed carrier does not authorize a filename-token edit'
source = 'error_corpus/validation_errors/Schlüssel.cha'
level = 'header'
claim = 'violates'
notes = 'Change only the preceding malformed carrier filename to decomposed Unicode. Finding and safe automatic editing remain separate obligations.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	Schlüssel,, audio, unlinked
*CHI:	hello .
@End
'''
+++

## Description

An unrelated malformed utterance does not change whether a media filename is
in Unicode NFC. A source-bound normalization proposal may still replace a
clean header's filename token, but must preserve the unrelated syntax fault.
That local token proof is not a certificate that the whole file is valid.
Recovery within the media header itself does not provide a clean carrier for
automatic filename normalization.

## CHAT Rule

Use the transcript's canonical Unicode NFC name in a syntactically complete
`@Media` declaration. Repair malformed header syntax and unmatched main-tier
brackets from the intended source content; normalization must not guess or
remove that content.
