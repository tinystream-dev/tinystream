// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("libdisplay-info", VERSION, &[]);
    let options = ["--default-library=static"];
    let url = "https://gitlab.freedesktop.org/emersion/libdisplay-info.git";

    b.once(&options.join(" "), |b| {
        b.git(url, VERSION);
        b.meson(&options);
    });

    b.licenses(&format!("{url} ({VERSION})"), &["LICENSE"]);
}
