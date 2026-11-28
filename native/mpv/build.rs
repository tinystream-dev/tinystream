// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("mpv", VERSION, &["ffmpeg", "placebo", "drm", "display-info"]);

    // Only what the app uses: libmpv's render API on OpenGL through EGL, VA-API (with a DRM render
    // node the app passes in, so it works on Wayland, X11 and offscreen alike) and NVDEC, and the
    // PipeWire and ALSA outputs. No windows of its own, scripting or discs, and no libass: text
    // subtitles are rustyass's.
    let options = [
        "--default-library=static",
        "--auto-features=disabled",
        "-Dlibmpv=true",
        "-Dcplayer=false",
        "-Dgpl=true",
        "-Dbuild-date=false",
        "-Dlua=disabled",
        "-Dlibass=disabled",
        "-Dgl=enabled",
        "-Dplain-gl=enabled",
        "-Degl=enabled",
        "-Ddrm=enabled",
        "-Dvaapi=enabled",
        "-Dvaapi-drm=enabled",
        "-Dcuda-hwaccel=enabled",
        "-Dcuda-interop=enabled",
        "-Dpipewire=enabled",
        "-Dalsa=enabled",
        "-Diconv=enabled",
        "-Dzlib=enabled",
        "-Dvector=enabled",
    ];

    let patches = ["patches/no-libass.patch"];
    let url = "https://github.com/mpv-player/mpv.git";
    let tag = format!("v{VERSION}");

    for patch in patches {
        println!("cargo::rerun-if-changed={patch}");
    }

    let applied = patches.map(|patch| fs::read_to_string(patch).unwrap()).concat();

    b.once(&format!("{} {applied}", options.join(" ")), |b| {
        b.git(url, &tag);

        for patch in patches {
            b.exec(b.cmd("git").args(["apply", "--whitespace=nowarn"]).arg(fs::canonicalize(patch).unwrap()));
        }

        b.meson(&options);
    });

    b.export("include", b.prefix.join("include"));
    b.licenses(&format!("{url} ({tag})"), &["Copyright", "LICENSE.GPL"]);
}
