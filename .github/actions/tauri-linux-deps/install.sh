#!/usr/bin/env bash
# Install the Tauri Linux packages: offline from a restored apt cache when the
# cache matches this runner image and package list exactly, otherwise from the
# mirror under time bounds. See action.yml for why both paths exist.
#
# Usage: install.sh CACHE_DIR CACHE_STATE
#   CACHE_DIR    a runner-writable directory that holds apt's package lists
#                (lists/) and downloaded packages (archives/)
#   CACHE_STATE  one of
#                  hit       CACHE_DIR was restored for this exact cache key
#                  miss      no cache entry exists for the key yet
#                  disabled  the runner image cannot be identified, so
#                            nothing may be restored or saved
#
# Writes `save=true` to $GITHUB_OUTPUT only when the mirror path filled an
# empty CACHE_DIR for a key that has no entry yet; otherwise `save=false`.
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "usage: install.sh CACHE_DIR CACHE_STATE" >&2
    exit 2
fi
cache_dir="$1"
cache_state="$2"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT must name the output file of this step}"

case "$cache_state" in
    hit | miss | disabled) ;;
    *)
        echo "install.sh: unknown cache state '$cache_state' (hit, miss or disabled)" >&2
        exit 2
        ;;
esac

action_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
packages=()
while IFS= read -r line; do
    case "$line" in
        '' | '#'*) ;;
        *) packages+=("$line") ;;
    esac
done < "$action_dir/packages.txt"
if [[ ${#packages[@]} -eq 0 ]]; then
    echo "install.sh: $action_dir/packages.txt names no packages" >&2
    exit 2
fi

lists="$cache_dir/lists"
archives="$cache_dir/archives"
mkdir -p "$lists/partial" "$archives/partial"

# apt reads and writes the cache directory instead of /var, so the cache
# action can save and restore it without root. The binary package caches are
# disabled so they can never be built from the system lists and paired with
# the cache's. Downloads run as root because the sandbox user cannot write a
# runner-owned directory.
apt_local=(
    -o "Dir::State::Lists=$lists"
    -o "Dir::Cache::Archives=$archives"
    -o Dir::Cache::pkgcache=
    -o Dir::Cache::srcpkgcache=
    -o APT::Sandbox::User=root
    -o APT::Keep-Downloaded-Packages=true
    -o DPkg::Lock::Timeout=120
)
# Recommends stay installed. Dropping them was measured on 2026-10-22 (about
# 50 fewer packages and 30 MB on Ubuntu 24.04), but it also drops runtime
# pieces such as at-spi2-core, xdg-desktop-portal-gtk and enchant-2, and the
# Linux desktop release bundles its AppImage from this same install. The cache
# makes the saving nearly free to forgo; changing what the shipped app is
# built against is not.
install_args=(install -y)

# BOUNDED, because a mirror can stall without failing; the three layers are
# described in action.yml.
mirror_options=(
    -o Acquire::Retries=3
    -o Acquire::http::Timeout=60
    -o Acquire::https::Timeout=60
)

install_offline() {
    sudo timeout --kill-after=30 300 \
        apt-get "${apt_local[@]}" --no-download "${install_args[@]}" "${packages[@]}"
}

install_from_mirror() {
    sudo timeout --kill-after=30 300 \
        apt-get "${apt_local[@]}" "${mirror_options[@]}" update &&
        sudo timeout --kill-after=30 900 \
            apt-get "${apt_local[@]}" "${mirror_options[@]}" "${install_args[@]}" "${packages[@]}"
}

record() {
    echo "save=$1" >> "$GITHUB_OUTPUT"
}

if [[ "$cache_state" == hit ]]; then
    if install_offline; then
        record false
        exit 0
    fi
    # The entry for this key already exists and cannot be replaced, so the
    # mirror install below saves nothing. A bound may have stopped dpkg
    # partway, so repair its state first rather than spend the mirror retry.
    echo "::warning::offline install from the restored apt cache failed; installing from the mirror"
    sudo dpkg --configure -a
fi

if ! install_from_mirror; then
    echo "::warning::apt-get failed or exceeded its time bound; retrying once"
    sudo dpkg --configure -a
    install_from_mirror
fi

if [[ "$cache_state" == miss ]]; then
    # apt ran as root; hand the directory back to the runner user so the cache
    # action can read it, and drop apt's locks and partial downloads.
    sudo chown -R "$(id -u):$(id -g)" "$cache_dir"
    rm -f "$lists/lock" "$archives/lock"
    rm -rf "${lists:?}/partial" "${archives:?}/partial"
    record true
else
    record false
fi
