+++
code = 'E243'
name = 'Unicode lexical scalar policy'

[[example]]
title = 'Ordinary and below-surrogate lexical controls'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	aab a·b a퟿b .
@End
'''

[[example]]
title = 'All private-use endpoints and former CLAN exemption endpoints'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ab ab ab ab a󰀀b a󿿽b a􀀀b a􏿽b .
@End
'''

[[example]]
title = 'Every Unicode noncharacter'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	a﷐b a﷑b a﷒b a﷓b a﷔b a﷕b a﷖b a﷗b a﷘b a﷙b a﷚b a﷛b a﷜b a﷝b a﷞b a﷟b a﷠b a﷡b a﷢b a﷣b a﷤b a﷥b a﷦b a﷧b a﷨b a﷩b a﷪b a﷫b a﷬b a﷭b a﷮b a﷯b a￾b a￿b a🿾b a🿿b a𯿾b a𯿿b a𿿾b a𿿿b a񏿾b a񏿿b a񟿾b a񟿿b a񯿾b a񯿿b a񿿾b a񿿿b a򏿾b a򏿿b a򟿾b a򟿿b a򯿾b a򯿿b a򿿾b a򿿿b a󏿾b a󏿿b a󟿾b a󟿿b a󯿾b a󯿿b a󿿾b a󿿿b a􏿾b a􏿿b .
@End
'''

[[example]]
title = 'Standard supplementary-plane scalar'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	a𐀀b .
@End
'''

[[example]]
title = 'Assigned high-BMP letters and symbols are not a forbidden block'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	a豈b aﬀb aﵐb aﷰb aﹰb a！b a～b a｟b a�b .
@End
'''

[[example]]
title = 'Neighbors outside private-use and noncharacter ranges'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	a豈b a﷏b aﷰb a🿽b a󯿽b .
@End
'''
+++

## Description

Lexical words reject Unicode private-use scalars and noncharacters. They do
not reject ordinary characters merely because they occur in a high BMP or
supplementary range. The existing control-character and CHAT punctuation
rules remain separate and unchanged. No character is silently normalized or
removed, and no CLAN-internal private-use exception applies.

## CHAT Rule

This is Chatter's lexical interchange policy, not a claim that private-use
characters or noncharacters are ill-formed Unicode. Private-use meanings depend
on a private agreement; noncharacters have no standard textual interpretation.
Use the intended standard transcription character rather than a private glyph
or internal sentinel.

Unicode defines private-use ranges U+E000–U+F8FF, U+F0000–U+FFFFD, and
U+100000–U+10FFFD. Noncharacters comprise U+FDD0–U+FDEF and the last two
scalars of each of the 17 planes. See the
[Unicode FAQ](https://www.unicode.org/faq/private_use.html).
This rule is not an assigned-character database or a general ban on unassigned
scalars. U+FFFD is not a noncharacter; it is allowed by this particular rule,
not a certification that an encoding conversion preserved the intended text.

## Expected Behavior

Every document parses without recovery and round-trips byte-exactly. Private-use
and noncharacter examples produce one located E243 per word; controls produce
none. The fixture includes every noncharacter, both ends of each private-use
range, and both ends of the former CLAN exemption. Word spans and diagnostic
categories are checked independently of the code-set observation snapshot.

## CHECK scope

The U+E000 CHECK 86 fixture still witnesses agreement on that one invalid word.
Chatter no longer copies CHECK's high-BMP blacklist or internal-markup exemption.
The previously recorded 21-Sep-2026 CHECK rejection of U+10000 is an intentional
divergence. No new CHECK observation is claimed by this policy change.
