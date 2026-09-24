+++
code = 'E243'
name = 'Standalone slash in word text'

[[example]]
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello [/] hello .
%com:	/
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
*CHI:	hello / hello .
%com:	/
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
*CHI:	<one two> [!] child [: children] .
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
*CHI:	<one /> [!] child [: /] .
@End
'''
+++

## Description

A standalone `/` is not a spoken word or a repetition annotation. The first
mutation removes the brackets from `[/]`, retaining a slash in the comment
tier as a free-text boundary control. The second pair replaces a nested word
and a replacement word with standalone slashes.

## CHAT Rule

CHAT marks repetition with the bracketed code `[/]`, not a bare slash.
See the CHAT manual's [Repetition section](https://talkbank.org/0info/manuals/CHAT.html#Repetition).
This rule addresses standalone word text, not slashes embedded in other
syntax or free-text comments. CHECK's standalone-slash check explicitly
exempts `%com:`.

## Expected Behavior

Parsing preserves the source. Validation reports E243 for every standalone
slash word, including nested and replacement words, without rewriting it
into an inferred repetition annotation. The `%com:` slash remains valid.

CHECK 21-Sep-2026 accepts examples 1 and 3. For example 2 it reports code 48
three times and code 11 once; example 4 receives six code-48 reports and two
code-11 reports. Chatter diagnoses each invalid word once rather than copying
CHECK's duplicate diagnostics.
