// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::{env, fs};

use native_support::Build;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const PUBLIC_DEFINES: &[(&str, Option<&str>)] = &[
    ("BOOST_ASIO_ENABLE_CANCELIO", None),
    ("BOOST_ASIO_NO_DEPRECATED", None),
    ("TORRENT_USE_OPENSSL", None),
    ("TORRENT_USE_LIBCRYPTO", None),
    ("TORRENT_SSL_PEERS", None),
    ("_FILE_OFFSET_BITS", Some("64")),
];

fn main() {
    let b = Build::new("libtorrent", VERSION, &["boost"]);
    let boost = PathBuf::from(env::var_os("DEP_BOOST_INCLUDE").unwrap());
    let openssl = openssl();

    let defines: Vec<String> =
        PUBLIC_DEFINES.iter().map(|(k, v)| v.map_or_else(|| k.to_string(), |v| format!("{k}={v}"))).collect();

    let defines = defines.join(" ");

    let url = format!(
        "https://github.com/arvidn/libtorrent/releases/download/v{VERSION}/libtorrent-rasterbar-{VERSION}.tar.gz"
    );

    b.once(&defines, |b| {
        b.fetch(&url, &[]);
        compile(&b.src, &boost, &openssl.0, &b.prefix.join("lib"));
    });

    b.licenses(&url, &["COPYING"]);
    b.export("include", b.src.join("include"));
    b.export("boost", &boost);
    b.export("lib", b.prefix.join("lib"));
    b.export("defines", &defines);
    b.export("openssl-include", env::join_paths(&openssl.0).unwrap());
    b.export("openssl-lib", env::join_paths(&openssl.1).unwrap());
}

/// Where OpenSSL's headers and libraries are, when they aren't on the compiler's default paths;
/// that's macOS, whose Homebrew OpenSSL is keg-only.
fn openssl() -> (Vec<PathBuf>, Vec<PathBuf>) {
    println!("cargo::rerun-if-env-changed=PKG_CONFIG_PATH");

    if env::var("CARGO_CFG_TARGET_VENDOR").is_ok_and(|v| v == "apple") {
        let found = pkg_config::Config::new().cargo_metadata(false).probe("openssl").unwrap_or_else(|e| {
            panic!("libtorrent needs OpenSSL; install it (brew install openssl@3 pkg-config): {e}")
        });

        return (found.include_paths, found.link_paths);
    }

    (Vec::new(), Vec::new())
}

fn cmake_list(cmake: &str, name: &str) -> Vec<String> {
    let start = cmake
        .find(&format!("set({name}\n"))
        .unwrap_or_else(|| panic!("libtorrent's CMakeLists.txt has no `set({name}` list"));

    let body = &cmake[start + name.len() + 5..];
    let body = &body[..body.find(')').unwrap()];
    body.lines().map(|l| l.split('#').next().unwrap().trim()).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

fn compile(lt: &Path, boost: &Path, openssl: &[PathBuf], out: &Path) {
    let cmake = fs::read_to_string(lt.join("CMakeLists.txt")).expect("libtorrent's CMakeLists.txt");
    let mut files: Vec<PathBuf> = Vec::new();

    for (list, dir) in [
        ("sources", "src"),
        ("kademlia_sources", "src/kademlia"),
        ("ed25519_sources", "src/ed25519"),
        ("try_signal_sources", "deps/try_signal"),
    ] {
        files.extend(cmake_list(&cmake, list).into_iter().map(|f| lt.join(dir).join(f)));
    }

    files.push(lt.join("src/pe_crypto.cpp"));

    fs::create_dir_all(out).unwrap();
    let mut build = cc::Build::new();

    build
        .cpp(true)
        .std("c++17")
        .files(&files)
        .include(lt.join("include"))
        .include(lt.join("deps/try_signal"))
        .flag("-isystem")
        .flag(boost.to_str().unwrap())
        .define("TORRENT_BUILDING_LIBRARY", None)
        .define("BOOST_EXCEPTION_DISABLE", None)
        .define("BOOST_ASIO_HAS_STD_CHRONO", None)
        .define("NDEBUG", None)
        .flag_if_supported("-fvisibility=hidden")
        .flag_if_supported("-fvisibility-inlines-hidden")
        .opt_level(2)
        .debug(false)
        .pic(true)
        .warnings(false)
        .cargo_metadata(false)
        .out_dir(out);

    for dir in openssl {
        build.include(dir);
    }

    for (k, v) in PUBLIC_DEFINES {
        build.define(k, *v);
    }

    build.compile("torrent-rasterbar");
}
