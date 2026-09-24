# pre_begin_editor_headers

Editor metadata headers belong before `@Begin`. Their free-text values survive
as `ColorWords`, `Window`, and `Font` model variants, not unknown headers. The
values below are retained in `corpus/reference/core/headers-pre-begin.cha`.
Isolating each header through the header-fragment API must preserve that model
identity; the reference-corpus fragment comparison checks this API contract.

## Input

```document
@UTF8
@Color words:	*CHI 1 32768 0 656 *MOT 1 656 33423 1311
@Window:	247_181_683_700_-1_-1_348_0_348_0
@Font:	Win:Courier New:14:25
@Begin
@End
```

## Expected CST

```cst
(source_file
  (full_document
    (utf8_header
      (newline))
    (color_words_header
      (color_words_prefix)
      (header_sep
        (colon)
        (tab))
      (free_text
        (rest_of_line))
      (newline))
    (window_header
      (window_prefix)
      (header_sep
        (colon)
        (tab))
      (free_text
        (rest_of_line))
      (newline))
    (font_header
      (font_prefix)
      (header_sep
        (colon)
        (tab))
      (free_text
        (rest_of_line))
      (newline))
    (begin_header
      (newline))
    (end_header
      (newline))))
```

## Metadata

- **Level**: header
- **Category**: header
