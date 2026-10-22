// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let b = Build::new("libva", VERSION, &[]);

    let options = ["-Dwith_x11=no", "-Dwith_glx=no", "-Dwith_wayland=no"];
    let lib = b.prefix.join("lib");
    let url = format!("https://github.com/intel/libva/archive/refs/tags/{VERSION}.tar.gz");

    b.once(&format!("{} shim", options.join(" ")), |b| {
        b.fetch(&url, &[]);
        b.meson(&options);

        for entry in fs::read_dir(&lib).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();

            if name.contains(".so") || name.ends_with(".dylib") {
                fs::remove_file(&path).unwrap();
            }
        }

        let pc = lib.join("pkgconfig/libva.pc");
        fs::write(&pc, fs::read_to_string(&pc).unwrap() + "Libs.private: -ldl\n").unwrap();
    });

    b.licenses(&url, &["COPYING"]);

    println!("cargo::rerun-if-changed=shim");

    for (name, src) in [("va", "shim/va.c"), ("va-drm", "shim/va_drm.c")] {
        cc::Build::new()
            .file(src)
            .include(b.prefix.join("include"))
            .flag("-fvisibility=hidden")
            .cargo_metadata(false)
            .out_dir(&lib)
            .compile(name);
    }
}
