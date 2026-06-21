#!/usr/bin/env python3
"""Extract the COMPLETE CLAN CHECK error-code reference directly from source.

The hand-maintained ``OSX-CLAN/CHECK-rules.md`` claims to be a "Complete Error
Reference" but drifted stale: it listed ~87 codes while ``check.cpp`` actually
defines 161 and emits ~135. A documentation-level chatter<->CHECK parity audit
that diffs against that stale doc therefore cannot see the missing codes (e.g.
119 "Missing word after code", the dangling-retrace check). This generator
reads ``check.cpp`` itself so the reference is reproducible and never silently
stale: re-run it whenever the CLAN sources are refreshed.

Two things are parsed from ``src/clan/check.cpp``:

1. ``check_mess()`` print switch (``case N: fprintf(fpout, "...");``) -> the
   message text(s) for every defined code. Dual cases of the form
   ``if (err_itm[0] == EOS) <generic> else <specific>`` yield two messages.
2. ``check_err(N, ...)`` call sites -> which codes are actually emitted, and how
   many trigger sites each has (rough proxy for how reachable a code is).

Outputs (paths given on the command line): a JSON model and a Markdown table.

Usage:
    extract_check_codes.py CHECK_CPP OUT_JSON OUT_MD
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

# A `case N:` label that is NOT commented out (leading `//` disqualifies it).
CASE_RE = re.compile(r"^\s*case\s+(\d+)\s*:")
COMMENTED_RE = re.compile(r"^\s*//")
# The switch ends at its `default:` label.
DEFAULT_RE = re.compile(r"^\s*default\s*:")
# First string-literal argument of an fprintf (the format string). Handles
# escaped quotes inside the literal.
FPRINTF_RE = re.compile(r'fprintf\s*\(\s*\w+\s*,\s*"((?:\\.|[^"\\])*)"')
# Trailing 0xNN byte arguments that back `%c%c%c` runs in unmatched-char msgs.
BYTE_ARG_RE = re.compile(r"0x([0-9A-Fa-f]{2})")
TRAILING_CODE_RE = re.compile(r"\s*\(%d\)\s*$")


@dataclass
class CheckCode:
    """One CLAN CHECK error code: its defined message(s) and emission sites."""

    code: int
    messages: list[str] = field(default_factory=list)
    call_sites: list[int] = field(default_factory=list)  # source line numbers


def _decode_byte_chars(fmt: str, line: str) -> str:
    """Substitute `%c%c%c` runs with the UTF-8 char from trailing 0xNN args.

    Messages like ``"Unmatched %c%c%c found on the tier."`` pass the three
    bytes of a single UTF-8 character (e.g. 0xE2,0x80,0xB9 -> the char) as
    separate ``%c`` args. Decode them so the reference shows the real glyph.
    """
    if "%c" not in fmt:
        return fmt
    byte_vals = [int(h, 16) for h in BYTE_ARG_RE.findall(line)]
    if not byte_vals:
        return fmt
    try:
        glyph = bytes(byte_vals).decode("utf-8")
    except UnicodeDecodeError:
        return fmt
    # Collapse each maximal run of %c into the decoded glyph (one glyph here).
    return re.sub(r"(?:%c)+", glyph, fmt)


def _clean(fmt: str, line: str) -> str:
    """Turn a raw fprintf format string into a human-readable message."""
    msg = fmt.replace('\\"', '"')
    msg = _decode_byte_chars(msg, line)
    msg = msg.replace("\\n", " ").replace("\\t", " ")
    msg = TRAILING_CODE_RE.sub("", msg)
    return " ".join(msg.split()).strip()


def parse_messages(lines: list[str]) -> dict[int, list[str]]:
    """Walk the check_mess() switch, mapping each code to its message text(s)."""
    codes: dict[int, list[str]] = {}
    current: int | None = None
    started = False
    for line in lines:
        if COMMENTED_RE.match(line):
            continue
        m = CASE_RE.match(line)
        if m:
            current = int(m.group(1))
            started = True
            codes.setdefault(current, [])
            # The label line itself may carry the fprintf (single-line cases).
            for fmt in FPRINTF_RE.findall(line):
                msg = _clean(fmt, line)
                if msg:
                    codes[current].append(msg)
            continue
        if started and DEFAULT_RE.match(line):
            break
        if current is not None:
            for fmt in FPRINTF_RE.findall(line):
                msg = _clean(fmt, line)
                if msg and msg not in codes[current]:
                    codes[current].append(msg)
    return codes


def parse_call_sites(lines: list[str], defined: set[int]) -> dict[int, list[int]]:
    """Map each code to the source lines that emit it.

    A code is emitted either directly via ``check_err(N, ...)`` or indirectly:
    helper functions ``return(N)`` an error code that the caller then feeds to
    ``check_err`` (e.g. 124 "remove unlinked" is only reached via ``return(124)``).
    ``return`` matches are restricted to codes that the switch actually defines,
    so ordinary ``return(0)`` / ``return TRUE`` control flow is not miscounted.
    """
    sites: dict[int, list[int]] = {}
    call_re = re.compile(r"check_err\s*\(\s*(\d+)")
    return_re = re.compile(r"return\s*\(?\s*(\d+)\s*\)?\s*;")
    for lineno, line in enumerate(lines, start=1):
        if COMMENTED_RE.match(line):
            continue
        for m in call_re.finditer(line):
            sites.setdefault(int(m.group(1)), []).append(lineno)
        for m in return_re.finditer(line):
            code = int(m.group(1))
            if code in defined:
                sites.setdefault(code, []).append(lineno)
    return sites


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: extract_check_codes.py CHECK_CPP OUT_JSON OUT_MD",
            file=sys.stderr,
        )
        return 2
    src_path, out_json, out_md = (Path(a) for a in sys.argv[1:4])
    lines = src_path.read_text(encoding="utf-8", errors="replace").splitlines()

    messages = parse_messages(lines)
    defined_codes = {code for code, msgs in messages.items() if msgs}
    call_sites = parse_call_sites(lines, defined_codes)

    codes: dict[int, CheckCode] = {}
    for code in sorted(set(messages) | set(call_sites)):
        codes[code] = CheckCode(
            code=code,
            messages=messages.get(code, []),
            call_sites=call_sites.get(code, []),
        )

    defined = [c for c in codes.values() if c.messages]
    emitted = [c for c in codes.values() if c.call_sites]
    defined_not_emitted = [c.code for c in defined if not c.call_sites]
    emitted_not_defined = [c.code for c in emitted if not c.messages]

    model = {
        "source": str(src_path),
        "summary": {
            "codes_defined_in_switch": len(defined),
            "codes_emitted_via_call_sites": len(emitted),
            "defined_but_never_emitted": sorted(defined_not_emitted),
            "emitted_but_no_message": sorted(emitted_not_defined),
        },
        "codes": [
            {
                "code": c.code,
                "messages": c.messages,
                "n_call_sites": len(c.call_sites),
                "call_site_lines": c.call_sites,
            }
            for c in codes.values()
        ],
    }
    out_json.write_text(json.dumps(model, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    md: list[str] = []
    md.append("# CLAN CHECK error codes (generated from check.cpp)\n")
    md.append(
        "Generated by `scripts/clan-sources/extract_check_codes.py` from "
        f"`{src_path}`. Do not edit by hand; re-run after a CLAN source refresh.\n"
    )
    md.append(
        f"- Codes defined in `check_mess()`: **{len(defined)}**\n"
        f"- Codes emitted by `check_err()` call sites: **{len(emitted)}**\n"
        f"- Defined but never emitted: {sorted(defined_not_emitted)}\n"
        f"- Emitted but no message text: {sorted(emitted_not_defined)}\n"
    )
    md.append("| code | emitted | message(s) |")
    md.append("|------|---------|------------|")
    for c in codes.values():
        emitted_flag = "yes" if c.call_sites else "no"
        msg = " / ".join(c.messages) if c.messages else "(no message; control case)"
        msg = msg.replace("|", "\\|")
        md.append(f"| {c.code} | {emitted_flag} | {msg} |")
    out_md.write_text("\n".join(md) + "\n", encoding="utf-8")

    print(
        f"defined={len(defined)} emitted={len(emitted)} "
        f"wrote {out_json} and {out_md}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
