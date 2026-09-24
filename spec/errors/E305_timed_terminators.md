+++
code = 'E305'
name = 'Missing terminator before timing media'

[[example]]
title = 'Timed reference control'
level = 'utterance'
source = 'media-bullets.cha'
claim = 'legal'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|sample|CHI|2;00.||||Child|||\n@ID:\teng|sample|MOT|||||Mother|||\n@Media:\tmedia-bullets, video\n@Comment:\tInline timing bullets on main tier and dependent tiers, inline_pic\n@Comment:\tConstructs: inline_bullet, timing_annotation, text_with_bullets,\n\ttext_with_bullets_and_pics, media bullets on dependent tiers, inline_pic\n*CHI:\thello there . \u00152041689_2042652\u0015\n%cod:\tthis is junk \u00152041689_2042652\u0015\n*MOT:\thow are you ? \u00155041689_5042652\u0015\n%act:\tfoo \u00152061689_2062652\u0015 bar \u00152061689_2062652\u0015\n%com:\tpic002 \u0015%pic:\"a18/image002.jpg\"\u0015\n@End\n"

[[example]]
title = 'Delete only the first period'
level = 'utterance'
source = 'media-bullets.cha'
claim = 'violates'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|sample|CHI|2;00.||||Child|||\n@ID:\teng|sample|MOT|||||Mother|||\n@Media:\tmedia-bullets, video\n@Comment:\tInline timing bullets on main tier and dependent tiers, inline_pic\n@Comment:\tConstructs: inline_bullet, timing_annotation, text_with_bullets,\n\ttext_with_bullets_and_pics, media bullets on dependent tiers, inline_pic\n*CHI:\thello there  \u00152041689_2042652\u0015\n%cod:\tthis is junk \u00152041689_2042652\u0015\n*MOT:\thow are you ? \u00155041689_5042652\u0015\n%act:\tfoo \u00152061689_2062652\u0015 bar \u00152061689_2062652\u0015\n%com:\tpic002 \u0015%pic:\"a18/image002.jpg\"\u0015\n@End\n"

[[example]]
title = 'Delete only the second question mark'
level = 'utterance'
source = 'media-bullets.cha'
claim = 'violates'
chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|sample|CHI|2;00.||||Child|||\n@ID:\teng|sample|MOT|||||Mother|||\n@Media:\tmedia-bullets, video\n@Comment:\tInline timing bullets on main tier and dependent tiers, inline_pic\n@Comment:\tConstructs: inline_bullet, timing_annotation, text_with_bullets,\n\ttext_with_bullets_and_pics, media bullets on dependent tiers, inline_pic\n*CHI:\thello there . \u00152041689_2042652\u0015\n%cod:\tthis is junk \u00152041689_2042652\u0015\n*MOT:\thow are you  \u00155041689_5042652\u0015\n%act:\tfoo \u00152061689_2062652\u0015 bar \u00152061689_2062652\u0015\n%com:\tpic002 \u0015%pic:\"a18/image002.jpg\"\u0015\n@End\n"
+++

## Description

A timing bullet does not replace the main-tier utterance terminator. These
examples retain the timed reference document's dependent tiers, picture and
continuation text. Each invalid variant deletes exactly one typed terminator
from `corpus/reference/content/media-bullets.cha`, retaining surrounding spaces
and all other bytes. The control is that reference document without changes.

## CHAT Rule

Outside Conversation Analysis mode an utterance requires a terminator; the
following timing bullet identifies media alignment rather than punctuation.
See the E305 specification for the terminator rule and its CA exception.

## Observed boundary

Both deletion variants parse without recovery and report E305 during validation.
The valid control roundtrips byte-exactly; invalid variants need not preserve
the doubled space left where their terminator was deleted during serialization.
Candidate generation itself preserves that space and every other source byte.

CHECK (21-Sep-2026) adds delimiter error 21 for each deletion. When run using
generated fixture basenames, it additionally reports filename error 157 for all
three examples because their authored `@Media` name is `media-bullets`. That
shared context mismatch is not evidence against the delimiter control.
