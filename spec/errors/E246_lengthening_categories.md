+++
code = 'E246'
name = 'Lengthening across word categories'

[[example]]
title = 'Ordinary word with two colons'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	no:: .
@End
'''

[[example]]
title = 'Ordinary word with three colons'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	no::: .
@End
'''

[[example]]
title = 'Filler with two colons'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&-uh:: .
@End
'''

[[example]]
title = 'Filler with three colons'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&-uh::: .
@End
'''

[[example]]
title = 'Phonetic filler with two colons'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&-uh::@u .
@End
'''

[[example]]
title = 'Phonetic filler with three colons'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&-uh:::@u .
@End
'''
+++

## Description

These paired category substitutions preserve material before the colons.
Ordinary and filler words therefore do not violate E246's placement rule.
The two/three-colon boundary extends the existing `lengthening` construct
spec's `no:::` example. Phonetic `@u` forms preserve their transcription as
opaque phonetic content rather than imposing orthographic prosody checks.
Each claim asserts only the absence of E246; CHECK acceptance is observed
separately, not inferred from that claim.

## CHAT Rule

The CHAT manual's section 9.9 places a lengthening colon after the prolonged
segment and identifies `&-` forms as filled pauses:
<https://talkbank.org/0info/manuals/CHAT.html>. The existing Chatter lengthening
construct permits repeated colons; these cases test that placement is not
confused with the number of colons or the word's category.

