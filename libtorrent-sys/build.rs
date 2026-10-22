// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/shim.cpp");
    println!("cargo:rerun-if-changed=src/shim.h");

    let dep = |key: &str| env::var(format!("DEP_TORRENT_RASTERBAR_{key}")).unwrap();
    let paths = |key: &str| env::split_paths(&dep(key)).filter(|p| !p.as_os_str().is_empty()).collect::<Vec<_>>();
    let mut shim = cxx_build::bridge("src/lib.rs");

    shim.file("src/shim.cpp")
        .std("c++17")
        .include(dep("INCLUDE"))
        .flag("-isystem")
        .flag(dep("BOOST"))
        .opt_level(2)
        .warnings(false);

    for define in dep("DEFINES").split(' ') {
        match define.split_once('=') {
            Some((k, v)) => shim.define(k, v),
            None => shim.define(define, None),
        };
    }

    for dir in paths("OPENSSL_INCLUDE") {
        shim.include(dir);
    }

    shim.compile("tinystream-libtorrent-shim");

    println!("cargo:rustc-link-search=native={}", dep("LIB"));
    println!("cargo:rustc-link-lib=static=torrent-rasterbar");

    for dir in paths("OPENSSL_LIB") {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    println!("cargo:rustc-link-lib=dylib=ssl");
    println!("cargo:rustc-link-lib=dylib=crypto");

    let apple = env::var("CARGO_CFG_TARGET_VENDOR").is_ok_and(|v| v == "apple");
    println!("cargo:rustc-link-lib=dylib={}", if apple { "c++" } else { "stdc++" });

    if apple {
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=SystemConfiguration");
    }
}
