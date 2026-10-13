# event_in_group

Event (`&=action`) inside an angle bracket group `<...>` with overlap annotation.
The closing `>` must NOT be part of the event description text.

Found in: aphasia-data, childes-eng-na-data, asd-data (570+ files).
Discovered by re2c parser parity testing (2026-03-30).

## Input

```main_tier
*CHI:	<&=laughs> [<] .
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
                (nonword_with_optional_annotations
                  (nonword
                    (event
                      (event_marker)
                      (event_segment)))))))
          (greater_than)
          (base_annotations
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
