+++
code = 'E202'
name = 'Missing form type after @'

[[example]]
level = 'word'
claim = { subsumed_by = 'E316' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello@ .
@End
'''

[[example]]
level = 'word'
claim = { subsumed_by = 'E203' }
notes = '''
Note: `@j` is recognized as a form type syntactically, but `j` is not a valid
form type value. This triggers E203 (InvalidFormType) rather than E202
(MissingFormType).
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	dog@j .
@End
'''

[[example]]
level = 'word'
title = 'a doubled @ is ONE defect, named by E203'
claim = { subsumed_by = 'E203' }
notes = '''
`word@@` ends with `@` AND carries a repeated `@` run, so it satisfies both
this file's dangling-marker rule and E203's at-most-one-suffix rule. The parser
names the repeated run as E203 with the run in hand, which is the specific
answer; adding E202 for the same word reports one defect twice, the generic
diagnostic buried under the specific one.

The suppression that stops the double already existed for E203-against-E203
and was never applied to the E202 branch four lines above it.
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	word@@ .
@End
'''
[[example]]
level = 'word'
claim = { subsumed_by = 'E316' }
title = 'the same dangling marker on another word (re-filed from E243_auto.md, whose example never demonstrated E243)'
notes = '''
The example `hell@` is rejected by grammar recovery with E316. The removed
raw-text classifier previously assigned E202; no dedicated parser diagnostic
is required to enforce rejection. E243 fires at the validation layer on parsed words containing
whitespace, control characters, or bullet markers. The paired
E243_word_controls.md source fixtures witness retained U+007F and U+0085;
U+0085 reaches both the whitespace and control-character checks.
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: @ character only allowed for form markers
@Comment:	Invalid: 'hell@' - @ in wrong position
*CHI:	hell@ .
@End
'''
[[example]]
level = 'word'
title = 'Valid form suffix inside a scoped group after multibyte text'
claim = 'legal'
notes = 'Seed for the following deletion. The defined @b form suffix is complete; accented text before it exercises byte-based source locations.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	café <baba@b again> [/] hello .
@End
'''

[[example]]
level = 'word'
title = 'Missing form suffix inside a scoped group after multibyte text'
claim = { subsumed_by = 'E316' }
notes = 'Derived from the preceding seed by deleting only b after @. Grouping, retracing and the multibyte prefix must not conceal the missing suffix. Exact location is not asserted by this code-presence claim.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	café <baba@ again> [/] hello .
@End
'''
[[example]]
level = 'word'
title = 'Valid form suffix in a phonology group'
claim = 'legal'
notes = 'Phonology-group seed; the following example deletes only the b in @b.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	café ‹baba@b again› hello .
@End
'''

[[example]]
level = 'word'
title = 'Missing form suffix in a phonology group'
claim = { subsumed_by = 'E316' }
notes = 'Single-character deletion from the preceding seed. A phonology group does not waive the requirement for a suffix after @.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	café ‹baba@ again› hello .
@End
'''

[[example]]
level = 'word'
title = 'Valid form suffix in a sign group'
claim = 'legal'
notes = 'Sign-group seed; the following example deletes only the b in @b.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	café 〔baba@b again〕 hello .
@End
'''

[[example]]
level = 'word'
title = 'Missing form suffix in a sign group'
claim = { subsumed_by = 'E316' }
notes = 'Single-character deletion from the preceding seed. A sign group does not waive the requirement for a suffix after @.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	café 〔baba@ again〕 hello .
@End
'''

[[example]]
level = 'utterance'
title = 'An extra question delimiter does not imply a missing form suffix'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello ? world .
@End
'''

[[example]]
level = 'utterance'
title = 'An extra exclamation delimiter does not imply a missing form suffix'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello ! world .
@End
'''

[[example]]
level = 'utterance'
title = 'A question delimiter inside a retraced scope is not a form marker'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<hello ? world> [/] goodbye .
@End
'''
+++

## Description

A word contains `@` at a position where a form type marker is expected, but
no valid form type follows. Tree-sitter produces an ERROR node at the `@`.

The redundant-delimiter controls contain no `@` and are legal only with
respect to E202. Their terminator or scope errors must still be rejected;
a misplaced `?` or `!` is not evidence of a missing form suffix.

The valid form types are declared in
`spec/form_markers/form_marker_registry.json` and rendered in the book's
[symbols chapter](../../book/src/chat-format/symbols.md). `@s` is the language
marker, a separate construct that is not a form type.

There used to be a copy of the list here, and it had drifted: it omitted `@u`.

## Expected Behavior

The tree-sitter parser rejects dangling suffix syntax through E316 without
rescanning ERROR text for an @ character. E202 remains a model-validation
diagnostic for admitted/imported word spelling; these are distinct boundaries.
The paired valid controls must parse and validate without diagnostics.
