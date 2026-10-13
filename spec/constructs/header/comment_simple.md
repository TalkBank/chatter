# comment_simple

`@Comment` header as free text.

## Input

```comment_header
@Comment:	note
```

## Expected CST

```cst
(comment_header
  (comment_prefix)
  (header_sep
    (colon)
    (tab))
  (text_with_bullets_and_pics
    (text_segment))
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
