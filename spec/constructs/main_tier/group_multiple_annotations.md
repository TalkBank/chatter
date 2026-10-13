# group_multiple_annotations

Angle bracket group with multiple space-separated scoped annotations.
All annotations must be collected as part of the same group, not split
into separate content items.

Found in: childes-eng-na, aphasia-data, asd-data (6,900+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```main_tier
*CHI:	<really> [?] [<] .
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
        (group_with_annotations
          (less_than)
          (contents
            (content_item
              (base_content_item
                (word_with_optional_annotations
                  (standalone_word
                    (word_body
                      (word_segment)))))))
          (greater_than)
          (base_annotations
            (whitespaces)
            (scoped_uncertain)
            (whitespaces)
            (indexed_overlap_precedes))))
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
