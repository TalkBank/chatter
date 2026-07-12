# overlap\_digit\_1

Overlap marker with digit 1 (⌊1) must lex as a single `overlap_point`
token, not as `overlap_point(⌊)` + `word_segment(1)`. The digit is part
of the overlap marker notation.

At the grammar level, [1-9] is accepted. The validator (E373) rejects
index 1, valid CHAT range is 2-9. This spec verifies the grammar
doesn't silently split the digit from the marker.

Custody (ideal overlap model, 2026-07-11): the marker's outer side
touches the whitespace/tab boundary, so it is a TOP-LEVEL content item
preceding the word; the single-token property is unchanged.

Regression gate for overlap_point regex change from `[2-9]?` to `[1-9]?`.

## Input

```main_tier
*CHI:	⌊1hello .
```

## Expected CST

```cst
(main_tier
  (star)
  speaker: (speaker)
  (colon)
  (tab)
  (tier_body
    content: (contents
      (overlap_point)
      (content_item
        (base_content_item
          (word_with_optional_annotations
            word: (standalone_word
              (word_body
                (word_segment))))))
      (whitespaces))
    ending: (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
