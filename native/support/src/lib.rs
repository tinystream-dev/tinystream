// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::SystemTime;

pub struct Build {
    name: String,
    version: String,
    work: PathBuf,
    downloads: PathBuf,

    pub src: PathBuf,

    pub prefix: PathBuf,
    pkgconfig: Vec<PathBuf>,
    path: Vec<PathBuf>,
    started: SystemTime,
}

impl Build {
    pub fn new(name: &str, version: &str, deps: &[&str]) -> Build {
        let started = SystemTime::now();
        let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

        let root =
            env::var_os("TINYSTREAM_NATIVE_DIR").map(PathBuf::from).unwrap_or_else(|| manifest.join("../../.native"));

        let work = root.join(format!("{name}-{version}"));
        let downloads = root.join("downloads");
        fs::create_dir_all(&work).expect("can't create the native build directory");
        fs::create_dir_all(&downloads).unwrap();
        let work = work.canonicalize().unwrap();
        let downloads = downloads.canonicalize().unwrap();
        let prefix = work.join("prefix");

        println!("cargo::rerun-if-changed=build.rs");
        println!("cargo::rerun-if-env-changed=TINYSTREAM_NATIVE_DIR");

        let mut pkgconfig = vec![prefix.join("lib/pkgconfig")];
        let mut path = vec![prefix.join("bin")];

        for dep in deps {
            let key = dep.to_uppercase().replace('-', "_");

            let var = |what| {
                env::var_os(format!("DEP_{key}_{what}"))
                    .unwrap_or_else(|| panic!("no metadata from `{dep}`; is it in [dependencies]?"))
            };

            pkgconfig.extend(env::split_paths(&var("PKGCONFIG")));
            path.extend(env::split_paths(&var("PATH")));
        }

        pkgconfig.dedup();
        path.dedup();

        Build {
            name: name.into(),
            version: version.into(),
            src: work.join("src"),
            work,
            downloads,
            prefix,
            pkgconfig,
            path,
            started,
        }
    }

    pub fn cflags(&self) -> String {
        println!("cargo::rerun-if-env-changed=TS_FFMPEG_NATIVE");
        let mut flags = "-O3 -fPIC".to_string();

        if env::var("TS_FFMPEG_NATIVE").is_ok_and(|v| v == "1") {
            flags.push_str(" -march=native");
        }

        flags
    }

    pub fn once(&self, config: &str, build: impl FnOnce(&Build)) {
        let marker = self.work.join(".done");
        println!("cargo::rerun-if-changed={}", marker.display());

        let config = format!("{config}\n{}", env::join_paths(&self.pkgconfig).unwrap().to_string_lossy());

        if fs::read_to_string(&marker).ok().as_deref() != Some(config.as_str()) {
            let _ = fs::remove_file(&marker);
            let _ = fs::remove_dir_all(&self.src);
            let _ = fs::remove_dir_all(&self.prefix);
            let _ = fs::remove_file(self.log());
            build(self);
            fs::write(&marker, &config).unwrap();

            File::options().write(true).open(&marker).unwrap().set_modified(self.started).unwrap();
        }

        self.export("prefix", &self.prefix);
        self.export("pkgconfig", env::join_paths(&self.pkgconfig).unwrap());
        self.export("path", env::join_paths(&self.path).unwrap());
    }

    pub fn export(&self, key: &str, value: impl AsRef<std::ffi::OsStr>) {
        println!("cargo::metadata={key}={}", value.as_ref().to_string_lossy());
    }

    /// Collects `files` from the source tree into `licenses/<name>.txt` next to the builds, for the
    /// third-party notices that ship with a release.
    pub fn licenses(&self, source: &str, files: &[&str]) {
        let dir = self.work.parent().unwrap().join("licenses");
        fs::create_dir_all(&dir).unwrap();
        let rule = "=".repeat(80);
        let mut out = format!("{rule}\n{} {}\n{source}\n{rule}\n", self.name, self.version);

        for file in files {
            let text = fs::read(self.src.join(file))
                .unwrap_or_else(|e| panic!("{} has no {file} to take its license from: {e}", self.name));

            out.push_str(&format!("\n--- {file} ---\n\n{}\n", String::from_utf8_lossy(&text).trim_end()));
        }

        fs::write(dir.join(format!("{}.txt", self.name)), out).unwrap();
    }

    pub fn fetch(&self, url: &str, members: &[&str]) {
        let archive = self.downloads.join(format!("{}-{}.tar.gz", self.name, self.version));

        if !archive.exists() {
            let part = archive.with_extension("part");
            self.exec(Command::new("curl").args(["-fL", "--retry", "3", "-o"]).arg(&part).arg(url));

            if !fs::read(&part).is_ok_and(|bytes| bytes.starts_with(&[0x1F, 0x8B])) {
                let _ = fs::remove_file(&part);
                panic!("{url} didn't serve a gzip archive (maybe a bot check page) for {}", self.name);
            }

            fs::rename(&part, &archive).unwrap();
        }

        fs::create_dir_all(&self.src).unwrap();

        self.exec(
            Command::new("tar")
                .arg("-xf")
                .arg(&archive)
                .arg("-C")
                .arg(&self.src)
                .arg("--strip-components=1")
                .args(members),
        );
    }

    /// Checks out `rev` (a branch, tag or commit) of `url`.
    pub fn git(&self, url: &str, rev: &str) {
        let clone = self.downloads.join(format!("{}-{}", self.name, self.version));

        if !clone.exists() {
            let part = clone.with_extension("part");
            let _ = fs::remove_dir_all(&part);
            let git = || {
                let mut cmd = Command::new("git");
                cmd.arg("-C").arg(&part);
                cmd
            };

            self.exec(Command::new("git").args(["init", "-q"]).arg(&part));
            self.exec(git().args(["fetch", "--depth", "1", url, rev]));
            self.exec(git().args(["checkout", "-q", "FETCH_HEAD"]));
            self.exec(git().args(["submodule", "update", "--init", "--recursive", "--depth", "1"]));
            fs::rename(&part, &clone).unwrap();
        }

        self.exec(Command::new("cp").arg("-a").arg(&clone).arg(&self.src));
    }

    pub fn cmd(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut cmd = Command::new(program);

        cmd.current_dir(&self.src)
            .env("PATH", prepend(&self.path, "PATH"))
            .env("PKG_CONFIG_PATH", prepend(&self.pkgconfig, "PKG_CONFIG_PATH"));

        cmd
    }

    pub fn exec(&self, cmd: &mut Command) {
        let log_path = self.log();
        let mut log = File::options().create(true).append(true).open(&log_path).unwrap();
        writeln!(log, "\n$ {cmd:?}").unwrap();

        let status = cmd
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .status()
            .unwrap_or_else(|e| panic!("can't run {:?} (needed to build {}): {e}", cmd.get_program(), self.name));

        if !status.success() {
            let log = fs::read_to_string(&log_path).unwrap_or_default();
            let lines: Vec<_> = log.lines().collect();

            panic!(
                "building {} failed: {cmd:?} exited with {status}. The end of {}:\n\n{}",
                self.name,
                log_path.display(),
                lines[lines.len().saturating_sub(40)..].join("\n"),
            );
        }
    }

    pub fn meson(&self, options: &[&str]) {
        self.exec(
            self.cmd("meson")
                .args(["setup", "build", "--libdir=lib", "--buildtype=release"])
                .arg(format!("--prefix={}", self.prefix.display()))
                .args(options),
        );

        self.exec(self.cmd("ninja").args(["-C", "build", &format!("-j{}", jobs()), "install"]));
    }

    pub fn configure(&self, args: &[&str]) {
        self.exec(self.cmd("./configure").arg(format!("--prefix={}", self.prefix.display())).args(args));
        self.make(&[]);
        self.make(&["install"]);
    }

    pub fn make(&self, args: &[&str]) {
        let mut cmd = self.cmd("make");

        match env::var_os("CARGO_MAKEFLAGS") {
            Some(flags) => cmd.env("MAKEFLAGS", flags),
            None => cmd.arg(format!("-j{}", jobs())),
        };

        self.exec(cmd.args(args));
    }

    fn log(&self) -> PathBuf {
        self.work.join("build.log")
    }
}

fn jobs() -> String {
    env::var("NUM_JOBS").unwrap_or_else(|_| "1".into())
}

fn prepend(dirs: &[PathBuf], var: &str) -> OsString {
    let mut all = dirs.to_vec();
    all.extend(env::split_paths(&env::var_os(var).unwrap_or_default()));
    env::join_paths(all).unwrap()
}
