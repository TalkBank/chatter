# time_duration_simple

`@Time Duration` header with strict time syntax.

## Input

```time_duration_header
@Time Duration:	17:30-18:00
```

## Expected CST

```cst
(time_duration_header
  (time_duration_prefix)
  (header_sep
    (colon)
    (tab))
  (time_duration_contents
    (strict_time))
  (newline))
```

## Metadata

- **Level**: header
- **Category**: header
