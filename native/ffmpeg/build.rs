// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

fn main() {
    let version = env!("CARGO_PKG_VERSION").strip_suffix(".0").unwrap_or(env!("CARGO_PKG_VERSION"));

    let apple = std::env::var("CARGO_CFG_TARGET_VENDOR").is_ok_and(|v| v == "apple");
    let nvdec = std::env::var_os("CARGO_FEATURE_NVDEC").is_some();

    let mut deps = vec!["nasm", "dav1d", "x264", "zimg", "opus", "mp3lame"];

    if !apple {
        deps.push("va");
    }

    if nvdec {
        deps.push("ffnvcodec");
    }

    let b = Build::variant("ffmpeg", version, nvdec.then_some("nvdec"), &deps);

    let lame = std::env::var("DEP_MP3LAME_PREFIX").expect("no metadata from lame-src");

    let mut args = vec![
        "--pkg-config-flags=--static".to_string(),
        format!("--extra-cflags={} -I{lame}/include", b.cflags()),
        format!("--extra-ldflags=-L{lame}/lib"),
        "--enable-static".into(),
        "--disable-shared".into(),
        "--enable-pic".into(),
        format!("--extra-libs=-l{} -lm", if apple { "c++" } else { "stdc++" }),
        "--enable-gpl".into(),
        "--enable-libx264".into(),
        "--enable-libdav1d".into(),
        "--disable-libdrm".into(),
        "--enable-libzimg".into(),
        "--enable-libopus".into(),
        "--enable-libmp3lame".into(),
        "--disable-programs".into(),
        "--disable-doc".into(),
        "--disable-network".into(),
        "--disable-indevs".into(),
        "--disable-outdevs".into(),
        "--disable-debug".into(),
        "--enable-optimizations".into(),
        "--enable-runtime-cpudetect".into(),
    ];

    if apple {
        args.push("--enable-videotoolbox".into());
    } else {
        args.push("--enable-vaapi".into());
    }

    if nvdec {
        args.extend(["--enable-ffnvcodec".into(), "--enable-nvdec".into(), "--enable-cuvid".into()]);
    }

    let url = format!("https://ffmpeg.org/releases/ffmpeg-{version}.tar.gz");

    b.once(&args.join(" "), |b| {
        b.fetch(&url, &[]);
        b.configure(&args.iter().map(String::as_str).collect::<Vec<_>>());
    });

    b.licenses(&url, &["LICENSE.md", "COPYING.GPLv2", "COPYING.LGPLv2.1"]);
}
