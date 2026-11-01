# pid_simple

`@PID` header carrying a persistent identifier, as every published TalkBank
transcript does. A pre-`@Begin` header, read by the text-header reader
every pre-`@Begin` header shares. Until 2026-09-08 no spec example
exercised it: its only coverage came from three reference-corpus files.

## Input

```pid_header
@PID:	11312/a-00013825-1
```

## Expected CST

```cst
(pid_header
  (pid_prefix)
  (header_sep
    (colon)
    (tab)
  )
  (free_text
    (rest_of_line)
  )
  (newline)
)
```

## Metadata

- **Level**: header
- **Category**: header
