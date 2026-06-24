#!/usr/bin/env bash
# Prepend a "Most people want the app" banner to a chatter GitHub Release's
# notes, so the release page leads non-programmers to the desktop app downloads
# rather than the curl command and the command-line tarball table.
#
# Idempotent: a second run is a no-op. Run it after the desktop apps are
# attached to the release (so the .dmg / .exe links resolve). The Release app
# banner workflow runs this automatically once "Release Desktop" succeeds; you
# can also run it by hand:  scripts/release/add_app_banner.sh v0.2.1
set -euo pipefail

tag="${1:?usage: add_app_banner.sh vX.Y.Z}"
repo="${CHATTER_REPO:-TalkBank/chatter}"

# Defend against a malformed tag reaching gh / the shell.
case "$tag" in
  v[0-9]*) : ;;
  *) echo "add_app_banner: expected a vX.Y.Z tag, got: $tag" >&2; exit 1 ;;
esac
ver="${tag#v}"

body="$(gh release view "$tag" --repo "$repo" --json body --jq '.body')"
if printf '%s' "$body" | grep -qF "Most people want the app"; then
  echo "add_app_banner: banner already present on $tag; nothing to do"
  exit 0
fi

base="https://github.com/$repo/releases/download/$tag"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT

{
  printf '## Most people want the app\n\n'
  printf 'Validate CHAT files in a normal window, no terminal needed. Download the **Chatter desktop app**:\n\n'
  printf -- '- **Mac (Apple Silicon, M1/M2/M3/M4):** [Chatter_%s_aarch64.dmg](%s/Chatter_%s_aarch64.dmg)\n' "$ver" "$base" "$ver"
  printf -- '- **Mac (Intel):** [Chatter_%s_x64.dmg](%s/Chatter_%s_x64.dmg)\n' "$ver" "$base" "$ver"
  printf -- '- **Windows:** [Chatter_%s_x64-setup.exe](%s/Chatter_%s_x64-setup.exe)\n\n' "$ver" "$base" "$ver"
  printf 'Open the downloaded file and follow the prompt (on Mac, drag Chatter to Applications). Full instructions: https://talkbank.github.io/chatter/install/\n\n'
  printf 'The command-line tool, the language server, and per-platform downloads are below.\n\n'
  printf -- '---\n\n'
  printf '%s' "$body"
} > "$tmp"

gh release edit "$tag" --repo "$repo" --notes-file "$tmp"
echo "add_app_banner: prepended app banner to $tag"
