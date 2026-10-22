// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

fn main() {
    let version = format!(
        "{}.{}.{:02}",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH").parse::<u32>().unwrap(),
    );

    let b = Build::new("nasm", &version, &[]);

    b.once("", |b| {
        b.fetch(&format!("https://www.nasm.us/pub/nasm/releasebuilds/{version}/nasm-{version}.tar.gz"), &[]);
        b.configure(&[]);
    });
}
