# CLAN Line Numbering

**Status:** Current
**Last modified:** {{git-dates:page}}

When you click "Open in CLAN" in the desktop app or press Enter in the TUI,
Chatter resolves the error location for the CLAN editor and sends an open
request. Successful coordinate resolution is not proof that the external
application launched or placed its cursor correctly. One important boundary
is that CLAN and Chatter count lines differently.

## Hidden Headers

Chatter's shared resolver follows these Mac CLAN hidden-header rules:

| Header | Purpose |
|--------|---------|
| `@UTF8` | Character encoding declaration |
| `@PID` | Persistent identifier |
| `@Font: CAfont:...` | CAfont metadata only; ordinary font headers remain visible |
| `@Color words:` | Color coding rules (the space is significant) |
| `@Window` | Window position/size |

Header names are matched case-insensitively and exactly, not as arbitrary
prefixes. The font value may have leading spaces or tabs before `CAfont:`.
`@ColorWords` is not the recognized `@Color words` spelling.

These headers are present in the `.cha` file but omitted from the display mapping.
CLAN's line numbers skip them entirely. A file that starts with `@UTF8` on
line 1 will show `@Begin` as "line 1" in CLAN's display, even though it's
actually line 2 in the file.

## What Chatter Does

Chatter automatically adjusts line numbers before sending to CLAN:

1. Use positive supplied line/column coordinates, or compute both from the
   source byte offset if either coordinate is unavailable or zero
2. Count how many hidden headers appear before that line
3. Subtract the hidden count to get CLAN's line number
4. Send the adjusted line number to CLAN

The TUI uses its retained source; the desktop resolver reads the requested
file. Coordinates and source must describe the same version of the document.
This mapping does not itself detect edits made since a diagnostic was produced.

## Edge Case: Errors on Hidden Lines

If an error is on a hidden header itself (e.g., a malformed `@UTF8` line),
CLAN cannot navigate to it because CLAN doesn't display that line. In this
case, "Open in CLAN" will show an error message explaining why.

## For Developers

The shared resolution logic lives in `talkbank_model::resolve_clan_location()`.
Both the TUI and the desktop app call this function, it resolves line/column
from byte offsets when needed and adjusts for hidden headers.

See [`clan_location.rs`](https://github.com/TalkBank/chatter/blob/main/crates/talkbank-model/src/errors/clan_location.rs)
for the implementation and tests.
