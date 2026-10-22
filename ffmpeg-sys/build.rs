// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

use bindgen::callbacks::{MacroParsingBehavior, ParseCallbacks};

const LIBS: [&str; 6] = ["avformat", "avfilter", "avcodec", "swresample", "swscale", "avutil"];

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:rerun-if-changed=wrapper.h");

    unsafe { env::set_var("PKG_CONFIG_PATH", env::var_os("DEP_FFMPEG_PKGCONFIG").unwrap()) };
    let mut includes = Vec::new();

    for lib in LIBS {
        let found = pkg_config::Config::new()
            .statik(true)
            .probe(&format!("lib{lib}"))
            .unwrap_or_else(|e| panic!("can't link lib{lib}: {e}"));

        includes.extend(found.include_paths);
    }

    includes.dedup();

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("bindings.rs");

    bindgen::builder()
        .header(manifest.join("wrapper.h").to_str().unwrap())
        .clang_args(includes.iter().map(|dir| format!("-I{}", dir.display())))
        .parse_callbacks(Box::new(IgnoreMacros(
            ["FP_NAN", "FP_INFINITE", "FP_ZERO", "FP_SUBNORMAL", "FP_NORMAL"].into(),
        )))
        .allowlist_function("(av|avcodec|avformat|avfilter|avio|avutil|swr|sws)_.*|avsubtitle_free")
        .allowlist_type("(AV|Swr|Sws).*")
        .allowlist_var("(AV|FF_|SWR_|SWS_|LIBAV|E[A-Z]).*")
        .impl_debug(true)
        .prepend_enum_name(false)
        .generate()
        .expect("generating FFmpeg bindings failed")
        .write_to_file(out)
        .expect("can't write FFmpeg bindings");
}

#[derive(Debug)]
struct IgnoreMacros(HashSet<&'static str>);

impl ParseCallbacks for IgnoreMacros {
    fn will_parse_macro(&self, name: &str) -> MacroParsingBehavior {
        if self.0.contains(name) { MacroParsingBehavior::Ignore } else { MacroParsingBehavior::Default }
    }
}
