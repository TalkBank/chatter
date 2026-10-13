# transcriber_simple

`@Transcriber` header with free-text value.

## Input

```transcriber_header
@Transcriber:	J Doe
```

## Expected CST

```cst
(transcriber_header
  (transcriber_prefix)
  (header_sep
    (colon)
    (tab))
  (free_text
    (rest_of_line))
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
