#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Packages a built desktop app as the release's tarball and AppImage, with their checksums:
#
#   desktop/packaging/package.sh <binary> <THIRD_PARTY_LICENSES> <version> <out-dir>
#
# Run it from the repository root; it needs curl, rsvg-convert (librsvg), desktop-file-validate
# (desktop-file-utils) and appstreamcli (AppStream), which check the metadata. appimagetool and the
# AppImage runtime are downloaded at the versions below into <out-dir>/.tools and checked against
# their SHA-256. A version with a suffix (0.40.0-abcdef0) is marked as a development release.

set -eu

appimagetool_url=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
appimagetool_sha256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
runtime_url=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
runtime_sha256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

binary=$1
licenses=$2
version=$3
out=$4
id=dev.tinystream.app
here=desktop/packaging
resources=desktop/app/resources
name=tinystream-desktop-$version-x86_64-linux
appimage=tinystream-desktop-$version-x86_64.AppImage

# Everything is dated to the commit, so packaging the same binary twice gives the same tarball.
SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}
export SOURCE_DATE_EPOCH
case "$version" in
    *-*) type=development ;;
    *) type=stable ;;
esac

fetch() {
    if ! echo "$3  $1" | sha256sum -c --status 2> /dev/null; then
        curl -fsSL --retry 3 -o "$1" "$2"
        echo "$3  $1" | sha256sum -c --quiet
    fi
    chmod +x "$1"
}

mkdir -p "$out/.tools"
fetch "$out/.tools/appimagetool" "$appimagetool_url" "$appimagetool_sha256"
fetch "$out/.tools/runtime" "$runtime_url" "$runtime_sha256"

stage=$out/$name
rm -rf "$stage" "$out/AppDir"
mkdir -p "$stage"

install -m755 "$binary" "$stage/tinystream-desktop"
install -m755 "$here/install.sh" "$stage/install.sh"
install -m644 "$resources/$id.desktop" LICENSE "$stage/"
install -m644 "$here/README.md" "$stage/README.md"
install -m644 "$licenses" "$stage/THIRD_PARTY_LICENSES"
sed -e "s/@VERSION@/$version/" -e "s/@DATE@/$(date -u -d "@$SOURCE_DATE_EPOCH" +%F)/" -e "s/@TYPE@/$type/" \
    "$here/$id.metainfo.xml" > "$stage/$id.metainfo.xml"
for size in 16 22 24 32 48 64 128 256 512; do
    mkdir -p "$stage/icons/hicolor/${size}x$size/apps"
    rsvg-convert -w "$size" -h "$size" "$resources/$id.svg" -o "$stage/icons/hicolor/${size}x$size/apps/$id.png"
done
install -Dm644 "$resources/$id.svg" "$stage/icons/hicolor/scalable/apps/$id.svg"
desktop-file-validate "$stage/$id.desktop"
appstreamcli validate --no-net --explain "$stage/$id.metainfo.xml"

# The AppImage is the same files, installed the way install.sh does into its /usr.
appdir=$out/AppDir
DESTDIR=$appdir "$stage/install.sh" --prefix /usr > /dev/null
sed -i 's|^Exec=.*|Exec=tinystream-desktop %U|' "$appdir/usr/share/applications/$id.desktop"
ln -s usr/bin/tinystream-desktop "$appdir/AppRun"
ln -s usr/share/applications/$id.desktop "$appdir/$id.desktop"
ln -s usr/share/icons/hicolor/scalable/apps/$id.svg "$appdir/$id.svg"
ln -s $id.svg "$appdir/.DirIcon"

find "$stage" "$appdir" -exec touch -h -d "@$SOURCE_DATE_EPOCH" {} +
tar -C "$out" --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$SOURCE_DATE_EPOCH" \
    -czf "$out/$name.tar.gz" "$name"

# appimagetool is itself an AppImage; extracting it spares the runner FUSE.
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 VERSION=$version "$out/.tools/appimagetool" --no-appstream \
    --runtime-file "$out/.tools/runtime" "$appdir" "$out/$appimage"

(cd "$out" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256" && sha256sum "$appimage" > "$appimage.sha256")
rm -rf "$stage" "$appdir"
echo "$out/$name.tar.gz"
echo "$out/$appimage"
