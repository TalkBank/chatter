# codeswitch_span_word_override

A word carrying its own `@s` marker inside a code-switch span, `<a word@s:x b> [@s:y]`.

The word's own marker WINS over the enclosing span. The two marks are not in
conflict and the inner one is not redundant: a transcript uses the span for the
matrix language of a switched stretch and marks individual donor-language items
inside it, which is the ordinary shape of lexical borrowing inside a
code-switched clause.

Resolution is therefore three-layered, innermost first: the word's own marker,
else the enclosing span, else the utterance. Each layer records a distinct
provenance (`word_explicit`, `span_explicit`, `default`), so a consumer can tell
which mark decided a given word rather than inferring it.

This is specified from attested usage rather than from the proposal, which had
suggested rejecting a word marker inside a span as ambiguous. Real transcripts
use it deliberately, so rejecting it would refuse valid data.

## Input

```main_tier
*CHI:	ik weet <how@s:fra to do> [@s:eng] .
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
                      (word_segment))
                    (word_lang_suffix)))))
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
