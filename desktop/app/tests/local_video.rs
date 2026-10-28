// SPDX-License-Identifier: AGPL-3.0-or-later

#![cfg(target_os = "linux")]

use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
#[ignore = "opens a native window; requires a hardware decoder and TINYSTREAM_TEST_VIDEO"]
fn local_playback_does_not_report_unrelated_gpu_driver_failures() {
    let video = std::env::var_os("TINYSTREAM_TEST_VIDEO").expect("set TINYSTREAM_TEST_VIDEO");
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let directory = std::env::temp_dir().join(format!("tinystream-local-video-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(directory.join("config/tinystream")).unwrap();
    std::fs::write(directory.join("config/tinystream/mpv.conf"), "ao=null\n").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_tinystream-desktop"))
        .arg("--video")
        .arg(video)
        .env("XDG_CONFIG_HOME", directory.join("config"))
        .env("XDG_STATE_HOME", directory.join("state"))
        .env("XDG_CACHE_HOME", directory.join("cache"))
        .env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}/no-session-bus", directory.display()))
        .env("RUST_LOG", "tinystream_desktop=debug,wgpu=warn,naga=warn")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(7));
    let running = child.try_wait().unwrap().is_none();
    if running {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let log = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    std::fs::remove_dir_all(directory).unwrap();
    assert!(running, "local playback exited prematurely:\n{log}");
    assert!(log.contains("decoding with vaapi") || log.contains("decoding with nvdec"), "no hardware decode:\n{log}");
    assert!(log.contains("first video frame drawn"), "no displayed video frame:\n{log}");
    assert!(!log.contains("Cannot load libcuda.so.1"), "unrelated CUDA probe reported a load error:\n{log}");
}
