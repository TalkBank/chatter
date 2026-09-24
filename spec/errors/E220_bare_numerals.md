+++
code = 'E220'
name = 'Bare numerals are not language-specific tone notation'

[[example]]
title = 'Mandarin bare numeral words'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	zho
@Participants:	CHI Target_Child
@ID:	zho|corpus|CHI|||||Target_Child|||
*CHI:	1 10 12 21 101 1001 10000 10001 100000001 100010001 .
@End
'''

[[example]]
title = 'Tone and homonym digits retain their language exemption'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	zho
@Participants:	CHI Target_Child
@ID:	zho|corpus|CHI|||||Target_Child|||
*CHI:	ma1 本1 .
@End
'''

[[example]]
title = 'A mixed language candidate cannot license a bare numeral'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	12@s:eng+zho .
@End
'''

[[example]]
title = 'Omission notation is not a bare numeral word'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	0word .
@End
'''
+++

**Status:** Current
**Last updated:** 2026-09-23 18:53 EDT

## Description

A language exemption for tone digits or numbered homonyms does not permit a
word consisting entirely of ASCII digits. Bare numeral words require E220 even
when a resolved language candidate otherwise allows embedded digits. Omission
notation keeps its separate typed category and is not classified as a numeral.

## CHAT Rule

The [CHAT manual](https://talkbank.org/0info/manuals/CHAT.html), section 8.8.4,
requires numbers to be written out according to their pronunciation. Its
language-code discussion permits tone numbers in Romanized words and homonym
numbers on Chinese characters; those are word components, not bare numerals.

## Notes

These are authored controls, not production attestations. Written Mandarin
number controls live in `corpus/reference/word-features/mandarin-number-spelling.cha`.
They exercise a number-generation API; neither this spec nor that lookup is an
automatic repair of invalid CHAT or an inference about how a number was spoken.
Unknown language resolution retains its existing diagnostic policy.

CHECK 21-Sep-2026, observed in file mode on 2026-09-23 at 18:55 EDT, emits
47 for each bare numeral in the first example and for the mixed-language
numeral. It accepts the tone/homonym and omission controls and the written
Mandarin reference. Chatter previously accepted both invalid examples; E220
now rejects them while preserving those controls.
