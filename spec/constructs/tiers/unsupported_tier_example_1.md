# unsupported_tier_example_1

Unknown dependent tier label captured by the fallback rule.

## Input

```utterance
*CHI:	okay .
%zzz:	custom notes
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
  (unsupported_dependent_tier
    (unsupported_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
