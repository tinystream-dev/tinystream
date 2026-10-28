// SPDX-License-Identifier: AGPL-3.0-or-later

//! Wayland has no portable always-on-top request. Use the compositor's own floating layout for
//! our PiP window on niri and sway, without installing rules or changing any other window.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub const APP_ID: &str = "dev.tinystream.Tinystream.PiP";

pub fn opened() {
    let niri = std::env::var_os("NIRI_SOCKET");
    let sway = std::env::var_os("SWAYSOCK");
    if niri.is_none() && sway.is_none() {
        return;
    }
    let _ = std::thread::Builder::new().name("picture in picture".into()).spawn(move || {
        let result = match (niri, sway) {
            (Some(path), _) => niri_float(Path::new(&path)),
            (_, Some(path)) => sway_float(Path::new(&path)),
            _ => Ok(()),
        };
        if let Err(error) = result {
            tracing::warn!("couldn't float the picture-in-picture window: {error}");
        }
    });
}

fn socket(path: &Path) -> io::Result<UnixStream> {
    let socket = UnixStream::connect(path)?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    Ok(socket)
}

fn ours(window: &Value) -> bool {
    window["pid"].as_u64() == Some(u64::from(std::process::id())) && window["app_id"].as_str() == Some(APP_ID)
}

fn niri_float(path: &Path) -> io::Result<()> {
    let mut events = socket(path)?;
    events.write_all(b"\"EventStream\"\n")?;
    let mut reader = BufReader::new(events);
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let event: Value = serde_json::from_str(&line)?;
        let window = event["WindowsChanged"]["windows"]
            .as_array()
            .and_then(|windows| windows.iter().find(|window| ours(window)))
            .or_else(|| event.get("WindowOpenedOrChanged")?.get("window").filter(|window| ours(window)));
        if let Some(id) = window.and_then(|window| window["id"].as_u64()) {
            let mut command = socket(path)?;
            writeln!(command, "{}", json!({"Action":{"MoveWindowToFloating":{"id":id}}}))?;
            let mut reply = String::new();
            BufReader::new(command).read_line(&mut reply)?;
            if serde_json::from_str::<Value>(&reply)?["Err"].is_string() {
                return Err(io::Error::other(reply));
            }
            return Ok(());
        }
    }
    Err(io::Error::new(io::ErrorKind::TimedOut, "PiP did not map"))
}

fn sway_request(socket: &mut UnixStream, kind: u32, payload: &[u8]) -> io::Result<()> {
    socket.write_all(b"i3-ipc")?;
    socket.write_all(&(payload.len() as u32).to_ne_bytes())?;
    socket.write_all(&kind.to_ne_bytes())?;
    socket.write_all(payload)
}

fn sway_reply(socket: &mut UnixStream) -> io::Result<Value> {
    let mut header = [0; 14];
    socket.read_exact(&mut header)?;
    let length = u32::from_ne_bytes(header[6..10].try_into().unwrap()) as usize;
    if &header[..6] != b"i3-ipc" || length > 16 * 1024 * 1024 {
        return Err(io::Error::other("invalid compositor reply"));
    }
    let mut payload = vec![0; length];
    socket.read_exact(&mut payload)?;
    serde_json::from_slice(&payload).map_err(io::Error::other)
}

fn sway_window(node: &Value) -> Option<u64> {
    if ours(node) {
        return node["id"].as_u64();
    }
    ["nodes", "floating_nodes"].into_iter().filter_map(|key| node[key].as_array()).flatten().find_map(sway_window)
}

fn sway_float(path: &Path) -> io::Result<()> {
    let mut events = socket(path)?;
    sway_request(&mut events, 2, b"[\"window\"]")?;
    sway_reply(&mut events)?;
    let mut command = socket(path)?;
    sway_request(&mut command, 4, b"")?;
    let tree = sway_reply(&mut command)?;
    let mut id = sway_window(&tree);
    let deadline = Instant::now() + Duration::from_secs(10);
    while id.is_none() && Instant::now() < deadline {
        let event = sway_reply(&mut events)?;
        id = sway_window(&event["container"]);
    }
    let id = id.ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "PiP did not map"))?;
    sway_request(&mut command, 0, format!("[con_id={id}] floating enable, sticky enable").as_bytes())?;
    let reply = sway_reply(&mut command)?;
    if reply.as_array().is_some_and(|results| results.iter().all(|result| result["success"] == true)) {
        Ok(())
    } else {
        Err(io::Error::other(reply.to_string()))
    }
}
