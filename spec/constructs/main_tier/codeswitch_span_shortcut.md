# codeswitch_span_shortcut

Multi-word code-switch span in its bare form, `<word word> [@s]`.

Every word inside the `<>` scope resolves the way a bare `word@s` does: with two
declared languages that is the non-primary one. With more it resolves to the
SECOND declared language, unless the current language is itself tertiary, in
which case it is left unresolved with a diagnostic asking for an explicit code.
The bare form is not a MISSING language code; it is its own resolution rule,
which is why the model carries it as a variant rather than an absent value.

This example pins the PARSE shape only, and the harness supplies a
single-language header. Resolution is covered separately, by
`span_words_resolve_to_the_span_language` in the utterance language-metadata
tests, which is where the two-language context lives.

## Input

```main_tier
*CHI:	ik weet niet <how to do it> [@s] .
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
        (group_with_annotations
          (less_than)
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
            (whitespaces)
            (content_item
              (base_content_item
                (word_with_optional_annotations
                  (standalone_word
                    (word_body
                      (word_segment)))))))
          (greater_than)
          (base_annotations
            (whitespaces)
            (code_switch_annotation
              (right_bracket)))))
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
