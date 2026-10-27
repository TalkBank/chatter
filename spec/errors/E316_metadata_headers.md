+++
code = 'E316'
name = 'Structured metadata header boundaries'

[[example]]
title = 'Multilingual metadata with Unicode situation text'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, fra, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Types:	long, toyplay, TD
@Situation:	café α — jeu partagé
*CHI:	hello .
@End
'''

[[example]]
title = 'Optional Types comma whitespace preserves the same fields'
level = 'header'
claim = 'legal'
notes = 'Change only the optional whitespace around the Types commas in the preceding control.'
chat = '''
@UTF8
@Begin
@Languages:	eng, fra, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Types:	long ,toyplay ,TD
@Situation:	café α — jeu partagé
*CHI:	hello .
@End
'''

[[example]]
title = 'Truncating Types after design removes the required activity and group'
level = 'header'
claim = 'violates'
notes = 'Delete only the range after the design field through the group field of the first control. Document recovery is not evidence that a typed missing activity or group carrier reached the decoder.'
chat = '''
@UTF8
@Begin
@Languages:	eng, fra, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Types:	long
@Situation:	café α — jeu partagé
*CHI:	hello .
@End
'''

[[example]]
title = 'Internal whitespace is not an activity atom'
level = 'header'
claim = 'violates'
notes = 'Insert one space inside toyplay in the first legal control. Optional comma whitespace cannot become field content.'
chat = '''
@UTF8
@Begin
@Languages:	eng, fra, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Types:	long, toy play, TD
@Situation:	café α — jeu partagé
*CHI:	hello .
@End
'''

[[example]]
title = 'Punctuation is not part of a Types group atom'
level = 'header'
claim = 'violates'
notes = 'Insert a hyphen inside TD in the first legal control. Types fields use the existing alphanumeric grammar, not free text.'
chat = '''
@UTF8
@Begin
@Languages:	eng, fra, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Types:	long, toyplay, T-D
@Situation:	café α — jeu partagé
*CHI:	hello .
@End
'''

[[example]]
title = 'Unicode free text cannot extend a Types group atom'
level = 'header'
claim = 'violates'
notes = 'Append é to TD in the first legal control. The same character remains legal in the Situation free text; these fields have different grammars.'
chat = '''
@UTF8
@Begin
@Languages:	eng, fra, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Types:	long, toyplay, TDé
@Situation:	café α — jeu partagé
*CHI:	hello .
@End
'''

[[example]]
title = 'A continuation cannot add a second Types triple'
level = 'header'
claim = 'violates'
notes = 'The Types production contains exactly three fields. A continuation cannot start another triple after the group field; preserving only the first triple would silently discard source.'
chat = '''
@UTF8
@Begin
@Languages:	eng, fra, spa
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Types:	long, toyplay, TD
	cross, narrative, SLI
@Situation:	café α — jeu partagé
*CHI:	hello .
@End
'''

+++

**Status:** Current
**Last updated:** {{git-dates:page}}

## Description

`@Types` contains three alphanumeric fields in design, activity, group order,
separated by commas. Optional spaces around those commas do not change the
fields. Truncating the header after design is not a complete Types declaration.
Internal whitespace, punctuation and non-ASCII text do not match those atoms.
The complete body contains one triple; a second triple on a continuation is
not part of the Types production.

The legal controls combine an ordered ISO 639-3 language list, the Types fields
and Unicode free text in `@Situation`. Unicode situation text is not a language
code or a Types field and must retain its authored spelling.

## Expected Behavior

The legal controls parse and validate without diagnostics. The truncated
declaration reports E316 through document-level recovery; it must not acquire
a fabricated activity, group or valid Types header. Recovery shape and exact
diagnostics may differ between parser backends.
