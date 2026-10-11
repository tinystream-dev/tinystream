#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Prints the THIRD_PARTY_LICENSES that ship with a release: the native libraries from <native-dir>,
# the system libraries a musl build links in, the Rust crates and the web UI's packages.
#
#   licenses/collect.sh <native-dir> <target> > THIRD_PARTY_LICENSES
#
# Run it after the release build, from the repository root, with cargo-about installed.

set -eu

native=$1
target=$2
here=$(dirname "$0")
repo=https://github.com/tinystream-dev/tinystream

part() {
    printf '\n\n'
    printf '#%.0s' $(seq 80)
    printf '\n# %s\n' "$1"
    printf '#%.0s' $(seq 80)
    printf '\n\n'
}

cat <<EOF
tinystream is free software, released under the GNU Affero General Public License
version 3 or later (see LICENSE). It stands on the shoulders of the projects below;
this file carries their copyright notices and licenses.

FFmpeg, x264 and LAME are linked into tinystream under the GNU (L)GPL.
The exact source of every native library is at the address listed with it, and
the source of tinystream itself, which you can use to rebuild and relink it, is
at $repo.
If any of those addresses stops working, open an issue there and we will send
you the source.
EOF

# Only what this build links: <native-dir> can hold licenses of libraries it no longer builds.
part "Native libraries"
for lib in $(cargo tree --locked -e normal --target "$target" --prefix none --format '{p}' |
    sed -n 's/^\([a-z0-9]*\)-src v.*/\1/p' | sort -u); do
    if [ -f "$native/licenses/$lib.txt" ]; then
        cat "$native/licenses/$lib.txt"
    fi
done

case $target in
*-musl)
    part "System libraries (linked statically into musl builds)"
    cat "$here"/musl/*.txt
    ;;
esac

part "Rust crates"
cargo about generate --workspace --locked --fail -c "$here/about.toml" "$here/about.hbs"

part "Web UI packages"
cat web/dist/client/third-party-licenses.txt
