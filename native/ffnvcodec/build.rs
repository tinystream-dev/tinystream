// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;

use native_support::Build;

fn main() {
    let tag = format!("n{}.0", env!("CARGO_PKG_VERSION"));
    let b = Build::new("ffnvcodec", &tag[1..], &[]);
    let prefix = format!("PREFIX={}", b.prefix.display());
    let url = format!("https://github.com/FFmpeg/nv-codec-headers/archive/refs/tags/{tag}.tar.gz");

    b.once("", |b| {
        b.fetch(&url, &[]);
        b.make(&["install", &prefix]);

        // Each header carries its own notice and there's no license file: gather the notices into one.
        let dir = b.src.join("include/ffnvcodec");
        let mut headers: Vec<_> = fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().file_name()).collect();
        headers.sort();
        let mut notices = String::new();

        for header in headers {
            let text = fs::read_to_string(dir.join(&header)).unwrap();
            let notice = &text[..text.find("*/").expect("a header without a notice") + 2];
            notices.push_str(&format!("{}:\n\n{notice}\n\n", header.to_string_lossy()));
        }

        fs::write(b.src.join("LICENSE"), notices).unwrap();
    });

    b.licenses(&url, &["LICENSE"]);
}
