// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

fn main() {
    let tag = format!("n{}.0", env!("CARGO_PKG_VERSION"));
    let b = Build::new("ffnvcodec", &tag[1..], &[]);
    let prefix = format!("PREFIX={}", b.prefix.display());
    let url = format!("https://github.com/FFmpeg/nv-codec-headers/archive/refs/tags/{tag}.tar.gz");

    b.once("", |b| {
        b.fetch(&url, &[]);
        b.make(&["install", &prefix]);
        b.notices(&[
            "include/ffnvcodec/dynlink_cuda.h",
            "include/ffnvcodec/dynlink_cuviddec.h",
            "include/ffnvcodec/dynlink_loader.h",
            "include/ffnvcodec/dynlink_nvcuvid.h",
            "include/ffnvcodec/nvEncodeAPI.h",
        ]);
    });

    b.licenses(&url, &["NOTICES"]);
}
