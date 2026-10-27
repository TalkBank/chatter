+++
code = 'E316'
name = 'Freecode without content'

[[example]]
title = 'A blank freecode on a main tier'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello [^  ] .
@End
'''

[[example]]
title = 'A nonempty freecode on a main tier'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello [^ note] .
@End
'''

[[example]]
title = 'A blank freecode on a annotated angle group'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<hello [^  ]> [?] .
@End
'''

[[example]]
title = 'A nonempty freecode on a annotated angle group'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<hello [^ note]> [?] .
@End
'''

[[example]]
title = 'A blank freecode on a quotation'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	“hello [^  ]” .
@End
'''

[[example]]
title = 'A nonempty freecode on a quotation'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	“hello [^ note]” .
@End
'''

[[example]]
title = 'A blank freecode on a phonological group'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	‹hello [^  ]› .
@End
'''

[[example]]
title = 'A nonempty freecode on a phonological group'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	‹hello [^ note]› .
@End
'''

[[example]]
title = 'A blank freecode on a sign group'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	〔hello [^  ]〕 .
@End
'''

[[example]]
title = 'A nonempty freecode on a sign group'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	〔hello [^ note]〕 .
@End
'''
+++

## Description

A recognized freecode must carry nonblank content between `[^ ` and `]`.
The annotation's open vocabulary does not make a missing payload meaningful.

## Expected Behavior

Report E316 for the recognized blank freecode, retain its complete source
spelling as invalid recovery, and keep the surrounding words and group syntax.
The same rule applies at every content depth. Model validation must retain
that invalidity after JSON readback. Sanitization must refuse a freecode with
no admitted payload rather than turning it into an ordinary redacted code.

Each invalid example differs from its legal control only by replacing the
freecode's text with a space. Every example contains the word `hello`; E306
(utterance without content) does not describe this annotation defect.
This does not classify arbitrary ERROR fragments as recognized freecodes.

## CHAT Rule

The freecode token carries free-form code text. Its grammar requires content;
whitespace alone does not supply that content. The primary parser already
rejected this payload but reported the unrelated empty-utterance code and
dropped the annotation. E316 records the generic invalid construct because
no dedicated freecode-content diagnostic exists.
