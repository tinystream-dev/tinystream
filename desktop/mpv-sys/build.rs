// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;

fn main() {
    unsafe { env::set_var("PKG_CONFIG_PATH", env::var_os("DEP_MPV_PKGCONFIG").expect("no metadata from mpv-src")) };

    pkg_config::Config::new()
        .statik(true)
        .cargo_metadata(true)
        .probe("mpv")
        .unwrap_or_else(|e| panic!("can't link libmpv: {e}"));
}
