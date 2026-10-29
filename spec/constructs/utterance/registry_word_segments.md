# registry_word_segments

The symbol registry permits a backslash at the start of a text segment. The second word has two adjacent text segments because its internal backslash begins a new segment.

These are authored lexical boundary controls, not claims of attestation.

## Input

```utterance
*CHI:	\café ca\fé .
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
                (word_segment)
                (word_segment))))))
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: utterance
- **Category**: word
