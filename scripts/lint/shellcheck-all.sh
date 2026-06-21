#!/usr/bin/env bash
#
# Run shellcheck over every tracked shell script in this repository and fail if
# any finding remains at shellcheck's default (strictest) severity. This is the
# canonical gate mandated by CLAUDE.md; the pre-commit hook and CI both call it,
# and this script must itself stay clean. (A comment must not start with the
# word "shellcheck" or the tool reads it as a malformed inline directive.)
#
# A "shell script" is every tracked *.sh plus every tracked extensionless file
# whose first line is a sh/bash/dash/ksh shebang. Gitignored sub-repos and
# vendored trees are not tracked in this repo, so `git ls-files` scopes the
# sweep correctly without explicit excludes. zsh scripts are intentionally NOT
# matched: shellcheck cannot analyze zsh, so shell scripts we own must be
# bash/sh to be gateable (see CLAUDE.md).
#
# Usage:
#   scripts/lint/shellcheck-all.sh            # check every tracked shell script
#   scripts/lint/shellcheck-all.sh FILE...    # check only the given files
#
# Kept byte-identical across the TalkBank repos that run their own shell-lint
# gate (this repo, chatter, talkbank-tools); each needs its own copy because the
# repos are independent git histories with independent CI. Apply any change
# here to all copies (they should stay md5-identical).
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# Return 0 if the first line of file "$1" is a shebang shellcheck can analyze.
has_shellcheckable_shebang() {
  local first_line
  IFS= read -r first_line < "$1" || return 1
  case "$first_line" in
    '#!'*) ;;
    *) return 1 ;;
  esac
  # Match bash/dash/ksh (substring, tolerates trailing args) and sh whether at
  # end of line or followed by interpreter args (`#!/bin/sh -e`,
  # `#!/usr/bin/env sh -e`). zsh is deliberately excluded (shellcheck cannot
  # analyze it); note `*/sh` does not match `.../zsh` because the char before
  # `sh` there is `z`, not `/`.
  case "$first_line" in
    *bash* | *dash* | *ksh*) return 0 ;;
    */sh | */sh\ * | *' sh' | *' sh '*) return 0 ;;
    *) return 1 ;;
  esac
}

# Decide whether one path is a shell script we should check.
is_shell_script() {
  case "$1" in
    *.sh) [ -f "$1" ] ;;
    *) [ -f "$1" ] && has_shellcheckable_shebang "$1" ;;
  esac
}

scripts=()
if [ "$#" -gt 0 ]; then
  # Explicit list (e.g. staged files from the pre-commit hook); keep only the
  # shell scripts among them.
  for path in "$@"; do
    if is_shell_script "$path"; then
      scripts+=("$path")
    fi
  done
else
  # Every tracked file in the repo.
  while IFS= read -r path; do
    if is_shell_script "$path"; then
      scripts+=("$path")
    fi
  done < <(git ls-files)
fi

if [ "${#scripts[@]}" -eq 0 ]; then
  echo "shellcheck-all: no shell scripts to check"
  exit 0
fi

echo "shellcheck-all: checking ${#scripts[@]} shell scripts at strictest severity..."
if shellcheck "${scripts[@]}"; then
  echo "shellcheck-all: all ${#scripts[@]} scripts clean"
else
  echo "shellcheck-all: FAILURES above. Fix them, or silence a specific line" >&2
  echo "  with a justified '# shellcheck disable=SCxxxx' comment and a reason." >&2
  exit 1
fi
