# fac_example_1

Facial-gesture tier as dependent text.

## Input

```utterance
*CHI:	hello .
%fac:	raised eyebrows
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
  (fac_dependent_tier
    (fac_tier_prefix)
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
