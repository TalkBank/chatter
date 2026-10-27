+++
code = 'E342'
name = 'Empty scoped content'

[[example]]
title = 'Retraced group with content'
level = 'utterance'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t<hello> [/] hello .\n@End\n"

[[example]]
title = 'Delete the only word inside a retraced group'
level = 'utterance'
claim = 'violates'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t<> [/] hello .\n@End\n"

[[example]]
title = 'Explained group with content'
level = 'utterance'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t<hello> [= greeting] hello .\n@End\n"

[[example]]
title = 'Delete the only word inside an explained group'
level = 'utterance'
claim = 'violates'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t<> [= greeting] hello .\n@End\n"
+++

## Description

A scoped annotation requires material inside its angle-bracket scope. These
paired examples delete only the group's sole word while retaining the closing
bracket, annotation, subsequent speech and utterance terminator.

## CHAT Rule

A scope identifies the material to which an annotation applies. An empty scope
has no such material. Parsing may recover missing content, but that recovery
is not a valid empty utterance model or a reason to discard its diagnostics.

## Observations

Both nonempty controls parse and validate without diagnostics and roundtrip
byte-exactly. Both empty variants report parse E342. A missing separator must
not become group content and trigger secondary E378 validation. Their recovered
serialization diverges from the input.
CHECK (21-Sep-2026) accepts the controls, reports 52 for the empty retrace and
73 for the empty explained scope.

An empty scope in source alone does not establish the shape of its recovered
model. These are recovery witnesses; inspect the actual model before drawing
serializer-coverage conclusions.
