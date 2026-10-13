# inline_bullet_continuation

Main tier utterance with continuation line containing a pause and media
bullet. The continuation content (newline + tab) is part of the same
utterance. Media bullets can appear inline within content (word timing),
not just as the final utterance-level timestamp.

grammar.js: media_url is in base_content_item; continuation is whitespace.

Found in: aphasia-data, childes-eng-na (1,400+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```main_tier
*PAT:	I think you could use new clothes→ 0_2633
	(1.0) 2633_4026
```

## Expected CST

```cst
(main_tier
  (star)
  (speaker)
  (colon)
  (tab)
  (tier_body
    (contents
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            (standalone_word
              (word_body
                (word_segment))))))
      (separator
        (non_colon_separator
          (level_pitch)))
      (whitespaces)
      (content_item
        (base_content_item
          (bullet
            (bullet_start)
            (bullet_timestamp)
            (bullet_timestamp)
            (bullet_end))))
      (whitespaces)
      (content_item
        (base_content_item
          (pause_token))))
    (utterance_end
      (whitespaces)
      (bullet
        (bullet_start)
        (bullet_timestamp)
        (bullet_timestamp)
        (bullet_end))
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
