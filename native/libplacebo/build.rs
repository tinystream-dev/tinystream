// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("libplacebo", VERSION, &[]);

    // OpenGL needs no shader compiler. glad and Jinja, which generate its loader, come as
    // submodules, so the build needs Python but no Python packages.
    let options = [
        "--default-library=static",
        "--auto-features=disabled",
        "-Dopengl=enabled",
        "-Ddovi=enabled",
        "-Ddemos=false",
        "-Dtests=false",
    ];

    let (url, tag) = ("https://code.videolan.org/videolan/libplacebo.git", format!("v{VERSION}"));

    b.once(&options.join(" "), |b| {
        b.git(url, &tag);
        b.meson(&options);
    });

    b.licenses(&format!("{url} ({tag})"), &["LICENSE"]);
}
