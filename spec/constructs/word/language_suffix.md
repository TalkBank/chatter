# language_suffix

Word with an opaque language suffix, `@s:code` or `@s:code1+code2`.
The immediate suffix token has no code-list CST children. Candidate decoding
belongs to its source-bound Rust adapter; registry validity is a later policy.

## Input

```main_tier
*CHI:	hao3@s:eng+zho .
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
                (word_segment))
              (word_lang_suffix)))))
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: word
