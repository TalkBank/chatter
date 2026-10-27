+++
code = 'E316'
name = 'Word and base annotation admission positions'

[[example]]
title = 'Word replacement precedes its scoped annotation'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [: hi] [!] .
@End
'''

[[example]]
title = 'Event accepts a scoped annotation'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	&=laughs [!] .
@End
'''

[[example]]
title = 'Action accepts a scoped annotation'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	0 [= points] .
@End
'''

[[example]]
title = 'Group accepts a scoped annotation'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	<hello> [!] .
@End
'''

[[example]]
title = 'Quotation accepts a scoped annotation'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	“hello” [!] .
@End
'''

[[example]]
title = 'Language precode precedes content'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	[- spa] hola .
@End
'''

[[example]]
title = 'Postcode follows the terminator'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello . [+ bch]
@End
'''

[[example]]
title = 'Language precode after word'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [- spa] .
@End
'''

[[example]]
title = 'Postcode before terminator after word'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [+ bch] .
@End
'''

[[example]]
title = 'Language precode after event'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	&=laughs [- spa] .
@End
'''

[[example]]
title = 'Postcode before terminator after event'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	&=laughs [+ bch] .
@End
'''

[[example]]
title = 'Language precode after action'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	0 [- spa] .
@End
'''

[[example]]
title = 'Postcode before terminator after action'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	0 [+ bch] .
@End
'''

[[example]]
title = 'Language precode after annotated group'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	<hello> [!] [- spa] .
@End
'''

[[example]]
title = 'Postcode before terminator after annotated group'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	<hello> [!] [+ bch] .
@End
'''

[[example]]
title = 'Language precode after quotation'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	“hello” [- spa] .
@End
'''

[[example]]
title = 'Postcode before terminator after quotation'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	“hello” [+ bch] .
@End
'''

[[example]]
title = 'Replacement cannot attach to event'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	&=laughs [: hello] .
@End
'''

[[example]]
title = 'Replacement cannot attach to action'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	0 [: hello] .
@End
'''

[[example]]
title = 'Replacement cannot attach to annotated group'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	<hello> [!] [: hello] .
@End
'''

[[example]]
title = 'Replacement cannot attach to quotation'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	“hello” [: hello] .
@End
'''

[[example]]
title = 'Replacement cannot follow a scoped word annotation'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [!] [: hi] .
@End
'''

[[example]]
title = 'A word cannot carry two replacements'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello [: hi] [: hey] .
@End
'''

[[example]]
title = 'A glued word annotation lacks required whitespace'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello[!] .
@End
'''

[[example]]
title = 'A glued replacement lacks required whitespace'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello[: hi] .
@End
'''

[[example]]
title = 'A glued event annotation lacks required whitespace'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	&=laughs[!] .
@End
'''

[[example]]
title = 'A glued action annotation lacks required whitespace'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	0[= points] .
@End
'''

[[example]]
title = 'A glued group annotation lacks required whitespace'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, spa
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	<hello>[!] .
@End
'''
+++

## Description

These isolated position controls use the words, event, group and postcode
forms documented by `corpus/reference/content/postcodes-and-freecodes.cha`,
`content/events-in-groups.cha`, `annotation/groups-regular.cha` and
`annotation/errors-and-replacements.cha`. Legal counterparts retain the same
constructs in their admitted positions.

## CHAT Rule

A language precode precedes utterance content; a postcode follows its terminator.
Neither is an ordinary scoped annotation. A word may have one replacement before
its base annotations. Events, actions, groups and quotations accept base
annotations, not word replacements. Every separate bracket annotation and
replacement requires preceding whitespace. An invalid run must not become clean
content by dropping a marker or moving it into another grammatical role.

The shared grammar's `word_with_optional_annotations`, `base_annotations`,
`nonword_with_optional_annotations`, `group_with_annotations` and
`quotation_with_optional_annotations` encode these position rules. Backend
recovery and diagnostic details may differ; native detection must not depend on
an unrelated invalid construct in the same utterance.
