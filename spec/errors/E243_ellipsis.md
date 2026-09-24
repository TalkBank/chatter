+++
code = 'E243'
name = 'Unicode ellipsis in word text'

[[example]]
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello +...
@End
'''

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello… +...
@End
'''

[[example]]
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello <one two> [!] child [: children] .
@End
'''

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello <one… two> [!] child [: children…] .
@End
'''
+++

## Description

The Unicode horizontal ellipsis (U+2026) is punctuation, not lexical word
content. Preserve it for diagnostics instead of silently normalizing it or
treating it as part of a spoken word. The paired mutations insert one U+2026
after an ordinary word, then at both a nested word and a replacement word.
The controls retain the same surrounding CHAT structure.

## CHAT Rule

For trailing off, CHAT uses the utterance terminator `+...`: a plus followed
by three ASCII periods. A Unicode ellipsis appended to a word does not encode
that terminator. See the CHAT manual's
[Trailing Off section](https://talkbank.org/0info/manuals/CHAT.html).
This spec addresses word text, not free-text comments or other prose fields.

## Expected Behavior

Parsing retains the original text. Validation reports E243 for each word
containing U+2026, including nested and replacement words; it does not rewrite
punctuation into a guessed utterance terminator. CHECK 48's U+2026 word check
is a separate shape from its pipe and semicolon checks. CHECK 21-Sep-2026
accepts both controls, reports 48 once on example 2, and twice on example 4.
