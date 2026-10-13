+++
code = 'E244'
name = 'Stress evidence across shortening recovery boundaries'

[[example]]
title = 'Separated stress in a complete shortened word'
level = 'word'
claim = 'legal'
notes = 'The complete shortening retains the selected stress positions. This is the control for the single-marker insertion and closing-delimiter deletion below.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈhelˌl(o) .
@End
'''

[[example]]
title = 'Duplicate primary stress in a complete shortened word'
level = 'word'
claim = 'violates'
notes = 'Insert only a second primary marker at the beginning of the preceding control. The complete source-bound word permits a local duplicate-marker proposal.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈˈhelˌl(o) .
@End
'''

[[example]]
title = 'Separated stress with a missing shortening close'
level = 'word'
claim = 'legal'
notes = 'Delete only the closing parenthesis from the first control. Absence of E244 is not whole-word validity: the missing delimiter must still be reported.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈhelˌl(o .
@End
'''

[[example]]
title = 'Duplicate primary stress with a missing shortening close'
level = 'word'
claim = 'violates'
notes = 'Delete only the closing parenthesis from the duplicate-marker control. The grammar retains a clean stress-bearing prefix word and reports the incomplete shortening outside it. The local proposal remains possible, but admission refuses the recovered utterance.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈˈhelˌl(o .
@End
'''

[[example]]
title = 'Separated stress with an interior shortening fault'
level = 'word'
claim = 'legal'
notes = 'Insert only a comma inside the complete shortening of example 1. The selected stress positions remain separated; this claim does not admit the malformed shortening.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈhelˌl(o,) .
@End
'''

[[example]]
title = 'Duplicate primary stress with an interior shortening fault'
level = 'word'
claim = 'violates'
notes = 'Insert only the same comma inside the complete shortening of example 2. A stress finding does not establish that the source-bound word carrier is structurally clean.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈˈhelˌl(o,) .
@End
'''
+++

## Description

Stress adjacency and source-carrier integrity answer different questions.
A retained sequence of typed stress markers can establish E244 while a
shortening is malformed. A missing close can leave the stress-bearing prefix
as a clean word, with the remaining text in a separate recovery region. Local
carrier integrity and admission of an edit to the utterance remain distinct.

The interior comma instead remains in an ERROR inside the shortening and its
containing word. The typed markers still establish the stress finding, but
the catalog refuses that recovered carrier before proposing any marker edit.
The four malformed cases retain E316; their model serialization is not a
byte-exact representation of the invalid source and must not be used as one.

## CHAT Rule

Stress markers must not be consecutive. A word-internal shortening must retain
both parentheses. Resolve structural recovery before applying local repairs;
neither the legal E244 control nor a catalog proposal certifies whole-file
validity.
