// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("libass", VERSION, &["nasm", "freetype", "fribidi", "harfbuzz"]);

    let args = [
        "--enable-static".to_string(),
        "--disable-shared".into(),
        "--with-pic".into(),
        "--disable-fontconfig".into(),
        "--disable-require-system-font-provider".into(),
        "--disable-libunibreak".into(),
        format!("CFLAGS={}", b.cflags()),
    ];

    let url = format!("https://github.com/libass/libass/releases/download/{VERSION}/libass-{VERSION}.tar.gz");

    b.once(&args.join(" "), |b| {
        b.fetch(&url, &[]);
        b.configure(&args.each_ref().map(String::as_str));
    });

    b.licenses(&url, &["COPYING"]);
}
