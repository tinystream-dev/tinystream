#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Fails unless every library <binary> links is one that every desktop Linux has, since the tarball and
# the AppImage bring none of their own. Libraries loaded at runtime (Vulkan and GPU drivers, libcuda,
# VA-API drivers, X11 and Wayland through winit) aren't linked, so they don't show up here.
#
#   desktop/packaging/check-libs.sh <binary>

set -eu

binary=$1
status=0

# No build directories, Nix store paths or the like.
paths=$(readelf -d "$binary" | sed -n 's/.*(\(RPATH\|RUNPATH\)).*\[\(.*\)\]/\2/p')
if [ -n "$paths" ]; then
    echo "$binary searches $paths for libraries" >&2
    status=1
fi

interpreter=$(readelf -l "$binary" | sed -n 's/.*\[Requesting program interpreter: \(.*\)\]/\1/p')
if [ "$interpreter" != /lib64/ld-linux-x86-64.so.2 ]; then
    echo "$binary is loaded by $interpreter, which other systems don't have" >&2
    status=1
fi

for lib in $(readelf -d "$binary" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p'); do
    case "$lib" in
        # glibc, and GCC's runtime for the native libraries' C++.
        libc.so.6 | libm.so.6 | libdl.so.2 | libpthread.so.0 | librt.so.1 | ld-linux-x86-64.so.2) ;;
        libgcc_s.so.1 | libstdc++.so.6) ;;
        # Graphics: mpv renders through EGL, the app through Vulkan.
        libEGL.so.1 | libGL.so.1 | libvulkan.so.1) ;;
        libwayland-client.so.0 | libwayland-egl.so.1 | libwayland-cursor.so.0 | libxkbcommon.so.0 | libxkbcommon-x11.so.0) ;;
        libX11.so.6 | libX11-xcb.so.1 | libxcb.so.1 | libXcursor.so.1 | libXrandr.so.2 | libXi.so.6 | libXext.so.6) ;;
        # Sound: mpv's PipeWire and ALSA outputs link their libraries rather than loading them.
        libasound.so.2 | libpipewire-0.3.so.0) ;;
        # Compression in FFmpeg, under the same name everywhere (unlike libbz2, which FFmpeg goes without).
        libz.so.1 | liblzma.so.5) ;;
        *)
            echo "$binary links $lib, which isn't on the list of libraries every desktop has" >&2
            status=1
            ;;
    esac
done

glibc=$(objdump -T "$binary" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1)
echo "$binary needs ${glibc#GLIBC_} or newer of glibc and links:"
readelf -d "$binary" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/  \1/p'
exit $status
