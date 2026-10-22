// SPDX-License-Identifier: AGPL-3.0-or-later

use native_support::Build;

fn main() {
    let version = env!("CARGO_PKG_VERSION").strip_suffix(".0").unwrap_or(env!("CARGO_PKG_VERSION"));
    let b = Build::new("lame", version, &[]);

    let apple = std::env::var("CARGO_CFG_TARGET_VENDOR").is_ok_and(|v| v == "apple");

    let mut args = vec![
        "--enable-static".to_string(),
        "--disable-shared".into(),
        "--with-pic".into(),
        "--disable-frontend".into(),
        "--disable-decoder".into(),
        "--disable-gtktest".into(),
        format!("CFLAGS={}", b.cflags()),
    ];

    if apple {
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        args.push(format!("--build={arch}-apple-darwin"));
    }

    let url = format!("https://downloads.sourceforge.net/project/lame/lame/{version}/lame-{version}.tar.gz");

    b.once(&args.join(" "), |b| {
        b.fetch(&url, &[]);

        if apple {
            let sym = b.src.join("include/libmp3lame.sym");
            let list = std::fs::read_to_string(&sym).unwrap();
            std::fs::write(&sym, list.replace("lame_init_old\n", "")).unwrap();
        }

        b.configure(&args.iter().map(String::as_str).collect::<Vec<_>>());
    });

    b.licenses(&url, &["COPYING"]);

    println!("cargo::rustc-link-search=native={}", b.prefix.join("lib").display());
}
