// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("libdrm", VERSION, &[]);
    let options = [
        "--default-library=static",
        "--auto-features=disabled",
        "-Dudev=false",
        "-Dtests=false",
        "-Dinstall-test-programs=false",
    ];
    let (url, tag) = ("https://gitlab.freedesktop.org/mesa/drm.git", format!("libdrm-{VERSION}"));

    b.once(&options.join(" "), |b| {
        b.git(url, &tag);
        b.meson(&options);
        b.notices(&["xf86drm.c", "xf86drmHash.c", "xf86drmRandom.c", "xf86drmSL.c", "xf86drmMode.c"]);
    });

    b.licenses(&format!("{url} ({tag})"), &["NOTICES"]);
}
