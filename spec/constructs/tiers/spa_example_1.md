# spa_example_1

Speech-act tier as dependent annotation text.

## Input

```utterance
*CHI:	can I go ?
%spa:	request
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
        (whitespaces))
      (utterance_end
        (question)
        (newline))))
  (spa_dependent_tier
    (spa_tier_prefix)
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
