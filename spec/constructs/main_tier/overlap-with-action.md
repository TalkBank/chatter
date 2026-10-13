# overlap_with_action

Action marker 0 within overlap markers should parse as separate elements

## Input

```main_tier
*CHI:	⌈ 0 &=laughter⌉ .
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
      (overlap_point)
      (whitespaces)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (zero)))))
      (whitespaces)
      (content_item
        (base_content_item
          (nonword_with_optional_annotations
            (nonword
              (event
                (event_marker)
                (event_segment))))))
      (overlap_point)
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: overlap_points
