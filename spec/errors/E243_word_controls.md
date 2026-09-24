+++
code = 'E243'
name = 'Non-printing controls inside lexical text'

[[example]]
level = 'word'
title = 'Printable word control'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n"

[[example]]
level = 'word'
title = 'Bell inserted within lexical text'
claim = { subsumed_by = ['E315', 'E316'] }
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thel\u0007lo .\n@End\n"

[[example]]
level = 'word'
title = 'Delete control inserted within lexical text'
claim = 'violates'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thel\u007Flo .\n@End\n"

[[example]]
level = 'word'
title = 'Next-line control inserted within lexical text'
claim = 'violates'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n"
+++

## Description

Non-printing control characters are not lexical material. The printable control
is paired with single-character insertions of U+0007, U+007F and U+0085 into
its word. U+0085 is also Unicode whitespace; acceptance by a recovery grammar
must not turn it into valid spoken text.

## CHAT Rule

Word text must not contain control characters. CHAT structural control markers
are separate syntax, not arbitrary bytes permitted inside words.

## Observations

U+0007 is rejected during parsing (E315 and E316); the recovered document does
not retain that character in a Word, so E243 is not duplicated. U+007F and
U+0085 produce E315 while retaining lexical text, and validation reports E243.
Both retained variants serialize byte-exactly; the U+0007 recovery does not.
The printable control parses and validates without diagnostics.

CHECK (21-Sep-2026) reports error 48 for U+0007 but accepts the control,
U+007F and U+0085 examples. Its silence for the latter two does not make
non-printing controls valid lexical material. These source fixtures witness
both the control-character and whitespace validation paths; no synthetic Word
construction is needed.
