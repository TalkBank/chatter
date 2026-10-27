#!/usr/bin/env bash
# Compile the real send2clan bridge against the Win32 doubles and run the
# executable/message-path admission contracts. Needs only a host C compiler; it never
# touches a registry, launches CLAN or requires Windows.
#
# Usage: run.sh [--negative-control]
#   --negative-control  compile the bridge as of an explicit Git revision
#                       (SEND2CLAN_CONTROL_REV) instead of the working tree,
#                       to show the harness detects the defect it guards.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
crate="$(cd "$here/../.." && pwd)"
cc="${CC:-cc}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

harness="$here/harness.c"
if [[ "${1:-}" == "--negative-control" ]]; then
    rev="${SEND2CLAN_CONTROL_REV:?set SEND2CLAN_CONTROL_REV to the revision to test}"
    mkdir -p "$work/csrc" "$work/tests/win32-doubles"
    git -C "$crate" show "$rev:crates/send2clan/csrc/send2clan.c" > "$work/csrc/send2clan.c"
    cp "$crate/csrc/send2clan.h" "$crate/csrc/send2clan_version.h" "$work/csrc/"
    cp "$here/harness.c" "$here/windows.h" "$here/shellapi.h" "$work/tests/win32-doubles/"
    harness="$work/tests/win32-doubles/harness.c"
fi

# Warnings are errors so a Win32-branch mistake surfaces at compile time;
# -Wswitch guards the exhaustive outcome classification. The sanitizers turn
# reads of an unadmitted buffer or overflow into failures.
"$cc" -std=c11 -Wall -Wextra -Wswitch -Werror \
    -fsanitize=address,undefined -fno-sanitize-recover=all -g \
    -I "$(dirname "$harness")" -I "$crate/csrc" \
    "$harness" -o "$work/harness"
"$work/harness"
