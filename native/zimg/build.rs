// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("zimg", VERSION, &[]);

    let args = [
        "--enable-static".to_string(),
        "--disable-shared".into(),
        "--with-pic".into(),
        format!("CXXFLAGS={}", b.cflags()),
    ];

    let (url, branch) = ("https://github.com/sekrit-twc/zimg.git", format!("release-{VERSION}"));

    b.once(&args.join(" "), |b| {
        b.git(url, &branch);
        let mut autogen = b.cmd("./autogen.sh");

        if env::var("CARGO_CFG_TARGET_VENDOR").is_ok_and(|v| v == "apple") {
            autogen.env("LIBTOOLIZE", "glibtoolize");
        }

        b.exec(&mut autogen);
        b.configure(&args.each_ref().map(String::as_str));
    });

    b.licenses(&format!("{url} ({branch})"), &["COPYING"]);
}
