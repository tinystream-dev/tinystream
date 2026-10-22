// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("fribidi", VERSION, &[]);
    let options = ["--default-library=static", "-Ddocs=false", "-Dbin=false", "-Dtests=false"];
    let url = format!("https://github.com/fribidi/fribidi/archive/refs/tags/v{VERSION}.tar.gz");

    b.once(&options.join(" "), |b| {
        b.fetch(&url, &[]);
        b.meson(&options);
    });

    b.licenses(&url, &["COPYING"]);
}
