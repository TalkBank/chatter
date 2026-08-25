# codeswitch_span_explicit

Multi-word code-switch span with an explicit language, `<word word> [@s:code]`.

Every word inside the `<>` scope takes the named language, exactly as if each
carried the `@s:code` suffix. The span is the multi-word counterpart of
`language_suffix`.

A single content item takes the annotation without angle brackets, as every
other scoped annotation does: `hallo [@s]` is well-formed and resolves exactly
like `hallo@s`. An earlier draft proposed rejecting it as redundant; that was
ruled against, because the general convention is that a scoped annotation may
attach to one item, and a span is not a special case of it.

A word inside the span may carry its OWN `@s` marker, and the word wins. This
is not redundancy to reject: attested transcripts use an explicitly-coded span
for the matrix language and mark individual loanwords inside it with the
donor language.

## Input

```main_tier
*CHI:	ik weet <how to do> [@s:eng] .
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
                      (word_segment)))))))
          (greater_than)
          (base_annotations
            (whitespaces)
            (code_switch_annotation
              (colon)
              (language_code)
              (right_bracket)))))
      (whitespaces))
    (utterance_end
      (period)
      (newline))))
```

## Metadata

- **Level**: main_tier
- **Category**: main_tier
