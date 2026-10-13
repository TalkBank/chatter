# int_example_1

Intonational tier as dependent text.

## Input

```utterance
*CHI:	hello .
%int:	rising contour
```

## Expected CST

```cst
(utterance
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
        (whitespaces))
      (utterance_end
        (period)
        (newline))))
  (int_dependent_tier
    (int_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (text_with_bullets
      (text_segment))
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
