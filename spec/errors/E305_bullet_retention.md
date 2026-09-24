+++
code = 'E305'
name = 'Missing terminator does not discard timing evidence'

[[example]]
title = 'Internal and terminal bullets remain distinct'
level = 'utterance'
source = 'bullet-retention.cha'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tbullet-retention, audio\n*CHI:\thello \u0015100_200\u0015 . \u0015200_300\u0015\n@End\n"

[[example]]
title = 'Two trailing bullets on an unterminated utterance'
level = 'utterance'
source = 'bullet-retention.cha'
claim = 'violates'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tbullet-retention, audio\n*CHI:\thello \u0015100_200\u0015 \u0015200_300\u0015\n@End\n"

[[example]]
title = 'An internal bullet before a terminator stays internal'
level = 'utterance'
source = 'bullet-retention.cha'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tbullet-retention, audio\n*CHI:\thello \u0015100_200\u0015 .\n@End\n"
[[example]]
title = 'CHECK multi option with the same two timing scopes'
level = 'utterance'
source = 'bullet-retention.cha'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@Options:\tmulti\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tbullet-retention, audio\n*CHI:\thello \u0015100_200\u0015 . \u0015200_300\u0015\n@End\n"

[[example]]
title = 'Manual spelling multiple with the same two timing scopes'
level = 'utterance'
source = 'bullet-retention.cha'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@Options:\tmultiple\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tbullet-retention, audio\n*CHI:\thello \u0015100_200\u0015 . \u0015200_300\u0015\n@End\n"

[[example]]
title = 'Explicit zero supplies content for the second timing scope'
level = 'utterance'
source = 'bullet-retention.cha'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tbullet-retention, audio\n*CHI:\thello \u0015100_200\u0015 0 . \u0015200_300\u0015\n@End\n"

[[example]]
title = 'Leading bullet has no preceding text scope'
level = 'utterance'
source = 'bullet-retention.cha'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n@Media:\tbullet-retention, audio\n*CHI:\t\u0015100_200\u0015 hello . \u0015200_300\u0015\n@End\n"

+++

## Description

A missing terminator does not authorize deletion of timing evidence. Internal
bullets, and a distinct grammar-routed terminal bullet when present, must retain
their values and source order in the model and serialized output.

## CHAT Rule

E305 requires the ordinary utterance terminator. The legal controls assert only
that this rule is satisfied; they do not adjudicate every placement policy for
internal bullets. The CHAT manual's Audio and Video Time Marks section assigns
a separate scope to each bullet, so silently collapsing multiple bullets loses
information: <https://talkbank.org/0info/manuals/CHAT.html#Media_Linking>.

## Observed boundary

CHECK (21-Sep-2026) reports 73 for the empty text scope between bullets in
examples 1 and 2, and 21 for the missing terminator in example 2. All generated
fixture basenames also incur 157 because they differ from the authored media
name. The inter-bullet rule is a separate adjudication question, not a claim of
this E305 spec. Regardless of that verdict, lowering must retain both bullets.

Examples 4 through 7 isolate the remaining questions. CHECK's internal `multi`
option suppresses 73 for example 4, but its current dependency file rejects that
option with 11. The manual's spelling `multiple` receives both 11 and 73.
Chatter preserves either option but reports E534; neither is a supported flag.
An explicit `0` before the second bullet avoids CHECK 73, whereas the leading
bullet in example 7 triggers it. Chatter reports E770 for that leading bullet;
the explicit-zero control remains diagnostic-free. Empty scopes between later
bullets remain a separate per-shape parity question; absence of E305 does not
settle them.
