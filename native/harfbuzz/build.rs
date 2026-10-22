// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("harfbuzz", VERSION, &["freetype"]);

    let options = [
        "--default-library=static",
        "-Dfreetype=enabled",
        "-Dglib=disabled",
        "-Dgobject=disabled",
        "-Dcairo=disabled",
        "-Dicu=disabled",
        "-Dchafa=disabled",
        "-Dtests=disabled",
        "-Ddocs=disabled",
        "-Dutilities=disabled",
        "-Dintrospection=disabled",
    ];

    let url = format!("https://github.com/harfbuzz/harfbuzz/archive/refs/tags/{VERSION}.tar.gz");

    b.once(&options.join(" "), |b| {
        b.fetch(&url, &[]);
        b.meson(&options);
    });

    b.licenses(&url, &["COPYING"]);
}
