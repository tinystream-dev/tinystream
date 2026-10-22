// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("dav1d", VERSION, &["nasm"]);
    let options = ["--default-library=static", "-Denable_tools=false", "-Denable_tests=false"];
    let url = "https://code.videolan.org/videolan/dav1d.git";

    b.once(&options.join(" "), |b| {
        b.git(url, VERSION);
        b.meson(&options);
    });

    b.licenses(&format!("{url} ({VERSION})"), &["COPYING"]);
}
