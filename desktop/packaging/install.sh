#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Installs the tinystream desktop app from this directory, for you alone by default:
#
#   ./install.sh                     into ~/.local
#   sudo ./install.sh --prefix /usr/local
#   ./install.sh --uninstall [--prefix DIR]
#
# DESTDIR stages the files elsewhere, for packagers; the desktop entry still points at the prefix.

set -eu

id=dev.tinystream.app
prefix=$HOME/.local
action=install

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix) prefix=${2:?--prefix needs a directory}; shift 2 ;;
        --prefix=*) prefix=${1#--prefix=}; shift ;;
        --uninstall) action=uninstall; shift ;;
        -h | --help) sed -n '4,10s/^# \{0,1\}//p' "$0"; exit 0 ;;
        *) echo "unknown option: $1 (see --help)" >&2; exit 1 ;;
    esac
done

case "$prefix" in
    /*) ;;
    *) prefix=$(pwd)/$prefix ;;
esac

here=$(cd "$(dirname "$0")" && pwd)
root=${DESTDIR:-}$prefix
bin=$root/bin
share=$root/share
docs=$share/doc/tinystream-desktop

icons() {
    (cd "$here/icons" && find hicolor -type f)
}

refresh() {
    if [ -n "${DESTDIR:-}" ]; then
        return
    fi
    if command -v update-desktop-database > /dev/null; then
        update-desktop-database -q "$share/applications"
    else
        echo "update-desktop-database isn't installed (it comes with desktop-file-utils), so tinystream:// links won't open the app until something else refreshes $share/applications" >&2
    fi
    if command -v gtk-update-icon-cache > /dev/null && [ -f "$share/icons/hicolor/index.theme" ]; then
        gtk-update-icon-cache -q -t "$share/icons/hicolor" || true
    fi
}

if [ "$action" = uninstall ]; then
    rm -f "$bin/tinystream-desktop" "$share/applications/$id.desktop" "$share/metainfo/$id.metainfo.xml"
    rm -rf "$docs"
    for icon in $(icons); do
        rm -f "$share/icons/$icon"
    done
    refresh
    echo "Removed tinystream from $prefix. Your servers and settings are kept in ~/.config/tinystream and ~/.local/state/tinystream."
    exit 0
fi

install -Dm755 "$here/tinystream-desktop" "$bin/tinystream-desktop"
install -d "$share/applications"
sed "s|^Exec=tinystream-desktop|Exec=\"$prefix/bin/tinystream-desktop\"|" "$here/$id.desktop" > "$share/applications/$id.desktop"
chmod 644 "$share/applications/$id.desktop"
install -Dm644 "$here/$id.metainfo.xml" "$share/metainfo/$id.metainfo.xml"
for icon in $(icons); do
    install -Dm644 "$here/icons/$icon" "$share/icons/$icon"
done
for doc in LICENSE README.md THIRD_PARTY_LICENSES; do
    install -Dm644 "$here/$doc" "$docs/$doc"
done
refresh

echo "Installed tinystream into $prefix; it's in your applications menu."
if [ -n "${DESTDIR:-}" ]; then
    exit 0
fi
case ":${XDG_DATA_DIRS:-/usr/local/share:/usr/share}:${XDG_DATA_HOME:-$HOME/.local/share}:" in
    *":$prefix/share:"* | *":$prefix/share/:"*) ;;
    *) echo "$prefix/share isn't in XDG_DATA_DIRS, so your desktop won't find the app's menu entry or tinystream:// links." >&2 ;;
esac
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) echo "To run tinystream-desktop from a terminal, add $prefix/bin to your PATH." >&2 ;;
esac
