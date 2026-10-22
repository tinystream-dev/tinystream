// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("freetype", VERSION, &[]);

    let options = [
        "--default-library=static",
        "-Dbrotli=disabled",
        "-Dbzip2=disabled",
        "-Dharfbuzz=disabled",
        "-Dpng=disabled",
        "-Dzlib=disabled",
    ];

    let url = format!("https://download.savannah.gnu.org/releases/freetype/freetype-{VERSION}.tar.gz");

    b.once(&options.join(" "), |b| {
        b.fetch(&url, &[]);
        b.meson(&options);
    });

    b.licenses(&url, &["LICENSE.TXT", "docs/FTL.TXT"]);
}
