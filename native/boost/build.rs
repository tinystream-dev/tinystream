// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("boost", VERSION, &[]);
    let dir = format!("boost_{}", VERSION.replace('.', "_"));
    let url = format!("https://archives.boost.io/release/{VERSION}/source/{dir}.tar.gz");

    b.once("license", |b| {
        b.fetch(&url, &[&format!("{dir}/boost"), &format!("{dir}/LICENSE_1_0.txt")]);
    });

    b.licenses(&url, &["LICENSE_1_0.txt"]);
    b.export("include", &b.src);
}
