# mod_example_1

Model tier with phonology-style grouped content.

## Input

```utterance
*CHI:	hello .
%mod:	abc
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
  (mod_dependent_tier
    (mod_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (pho_groups
      (pho_group
        (pho_words
          (pho_word))))
    (newline)))
```

## Metadata

- **Level**: tier
- **Category**: tiers
