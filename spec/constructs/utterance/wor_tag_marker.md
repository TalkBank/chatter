# wor_tag_marker

Authored combination of the existing `with_mor` main-tier word and the
supported `%wor` `tag_marker` construct. Both tiers retain the declared
ending or separator. This is a parsing control, not external corpus attestation.

## Input

```utterance
*CHI:	hello „ .
%wor:	hello „ .
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
        (separator
          (non_colon_separator
            (tag_marker)))
        (whitespaces))
      (utterance_end
        (period)
        (newline))))
  (wor_dependent_tier
    (wor_tier_prefix)
    (tier_sep
      (colon)
      (tab))
    (wor_tier_body
      (wor_word_item
        (standalone_word
          (word_body
            (word_segment))))
      (whitespaces)
      (tag_marker)
      (whitespaces)
      (period)
      (newline))))
```

## Metadata

- **Level**: utterance
- **Category**: utterance
