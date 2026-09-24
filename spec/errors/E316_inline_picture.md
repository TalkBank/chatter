+++
code = 'E316'
name = 'Malformed inline picture reference'

[[example]]
title = 'A complete inline picture reference'
level = 'tier'
claim = 'legal'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tlook .
%com:\tpicture \u0015%pic:"image.jpg"\u0015
@End
"""

[[example]]
title = 'Delete only the picture filename'
level = 'tier'
claim = 'violates'
notes = 'Keep both quotes and both NAK delimiters, deleting only image.jpg from the control.'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tlook .
%com:\tpicture \u0015%pic:""\u0015
@End
"""

[[example]]
title = 'Delete only the closing picture quote'
level = 'tier'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tlook .
%com:\tpicture \u0015%pic:"image.jpg\u0015
@End
"""

[[example]]
title = 'Delete only the closing picture NAK delimiter'
level = 'tier'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|corpus|CHI|||||Target_Child|||
*CHI:\tlook .
%com:\tpicture \u0015%pic:"image.jpg"
@End
"""
+++

**Status:** Current
**Last updated:** 2026-09-23 23:29 EDT

## Description

An inline picture reference has a nonempty quoted filename enclosed by NAK
delimiters. Deleting its filename or a closing delimiter must not silently
turn the malformed reference into ordinary comment text.

The valid control parses without diagnostics and roundtrips byte-exactly.
All three deletion variants report E316 during parsing, with no validation
diagnostics and divergent recovered serialization. They become ERROR nodes,
not admitted `inline_pic` tokens, and therefore do not prove that the picture
filename admission guard is redundant.

CHECK (21-Sep-2026), observed on 2026-09-23, reports an error for the missing
closing NAK but not the empty filename or missing closing quote. Its
`check_ParseWords` hidden-token branch checks whether NAK scanning terminated
and reports error 59 if it did not; that branch does not validate the picture
payload on a dependent tier. Chatter retains its stricter structured-picture
syntax. CHECK silence here is not evidence of a usable picture reference.

## CHAT Rule

Retain the complete inline picture token, including the filename, paired
quotes and both media delimiters. These examples derive from the same valid
control by one deletion each; they do not require the picture file to exist
to test syntax.
