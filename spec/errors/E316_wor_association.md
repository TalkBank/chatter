+++
code = 'E316'
name = 'Unassociated word timing'

[[example]]
title = "Partial timing control"
level = "tier"
claim = "legal"
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|sample|CHI|||||Child|||
*CHI:\thello world .
%wor:\thello \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "Leading bullet has no word"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|sample|CHI|||||Child|||
*CHI:\thello world .
%wor:\t\u0015100_200\u0015 hello world .
@End
"""

[[example]]
title = "Bullet after separator has no word"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|sample|CHI|||||Child|||
*CHI:\thello world .
%wor:\thello , \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "A second bullet cannot replace the first"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|sample|CHI|||||Child|||
*CHI:\thello world .
%wor:\thello \u0015100_200\u0015 \u0015300_400\u0015 world .
@End
"""
+++

## Description

A word timing bullet belongs to its immediately preceding untimed word.
A leading bullet, a bullet after a separator, or a second bullet has no such
word. Recovery retains the complete original WOR tier; it must not drop the
bullet or overwrite an earlier interval. Untimed words remain legal.

## Expected Behavior

Report E316 and retain an explicit unparsed tier at the file boundary.
A fragment requiring a parsed WorTier rejects. Recovery is not timing evidence.

## CHAT Rule

The shared [WOR timing contract](../../book/src/architecture/wor-timing.md)
associates an optional timing interval with each word position.
