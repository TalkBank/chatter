+++
code = 'E316'
name = 'Unparsable header body'

[[example]]
title = 'A labelled gem is a gem header'
level = 'header'
claim = 'legal'
notes = 'The control for the malformed gem below: the colon, its tab and a label.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@G:	story
*CHI:	hello world .
@End
'''

[[example]]
title = 'A gem colon followed by a space is unparsable, not a labelled gem'
level = 'header'
claim = 'violates'
notes = 'Replace only the tab after the colon of the control with a space. The separator is a colon and a tab, so what follows the colon is unparsable text: the header is kept as unknown with its text, never a gem labelled from part of it and never the bare marker.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@G: story
*CHI:	hello world .
@End
'''

[[example]]
title = 'A tab where a page number belongs is unparsable, not an empty header'
level = 'header'
claim = 'violates'
notes = 'The body after the separator is one tab. A tab is not separator provenance (only spaces are), and it is not a page number, so the body is unparsable text rather than blank: the header is kept as unknown with its text, never the empty state (E553).'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Page:\t\t
*CHI:	hello world .
@End
"""

[[example]]
title = 'A page number with a letter after it is unparsable, not the number before it'
level = 'header'
claim = 'violates'
notes = 'A page number is digits. The body reads the digit and leaves the letter beside it as unparsable text, so the header is kept as unknown with its whole text, never a page header built from the digit alone.'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Page:\t1x
*CHI:	hello world .
@End
"""

+++

**Status:** Current
**Last updated:** {{git-dates:page}}

## Description

A header body that its grammar cannot read is unparsable content, whatever
header it belongs to. That includes text where a gem's separator belongs (a
colon followed by a space), a body that is not blank but holds only a
character the body's grammar does not admit (a tab where a page number
belongs), and a body whose grammar reads only its first part and leaves the
rest unparsable beside it (a letter after a page number, a second word in a
`@Types` field, a control character in the middle of any header's text).

## Expected Behavior

Both parser backends report E316 and keep the line as one unknown header
with its text, the recovery every other malformed header gets, which
validation then names as an unknown or malformed header (E525). Neither
backend lowers the line to a header of its kind built from part of the
text, to the empty state, or to a gem's bare marker, and neither drops the
line from the model. In particular, the part of a body before the
unparsable text is never kept as the header's content: the rest of the
line would be lost without a trace in the model.
