// SPDX-License-Identifier: AGPL-3.0-or-later

//! Security keys through `/dev/hidraw*`: any HID device whose report descriptor declares the FIDO
//! usage page.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::Duration;

use crate::passkeys::{Error, Hid};

const FIDO_USAGE_PAGE: u32 = 0xF1D0;
const FIDO_USAGE_CTAPHID: u32 = 0x01;

struct Hidraw(File);

impl Hid for Hidraw {
    fn write(&mut self, report: &[u8; 64]) -> io::Result<()> {
        let mut out = [0; 65];
        out[1..].copy_from_slice(report);
        self.0.write_all(&out)
    }

    fn read(&mut self, timeout: Duration) -> io::Result<Option<[u8; 64]>> {
        let mut poll = libc::pollfd { fd: self.0.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: `poll` points at one valid pollfd for the length of the call.
        let ready = unsafe { libc::poll(&mut poll, 1, millis) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            return if error.kind() == io::ErrorKind::Interrupted { Ok(None) } else { Err(error) };
        }
        if ready == 0 {
            return Ok(None);
        }
        if poll.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let mut report = [0; 64];
        let n = self.0.read(&mut report)?;
        if n == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        Ok(Some(report))
    }
}

/// Opens every security key plugged in. Keys we may not open only matter when no other key works.
pub fn find() -> Result<Vec<Box<dyn Hid>>, Error> {
    let mut keys: Vec<Box<dyn Hid>> = vec![];
    let mut denied = None;
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else { return Ok(keys) };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Ok(descriptor) = fs::read(entry.path().join("device/report_descriptor")) else { continue };
        if !is_fido(&descriptor) {
            continue;
        }
        let path = Path::new("/dev").join(&name);
        match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(file) => keys.push(Box::new(Hidraw(file))),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => denied = Some(path),
            Err(_) => {},
        }
    }
    match denied {
        Some(path) if keys.is_empty() => Err(Error::Failed(format!(
            "The app isn’t allowed to use your security key ({}). Most distributions allow it through \
             udev’s FIDO rules: install your distribution’s libfido2 or u2f-udev package, then unplug \
             the key and plug it back in.",
            path.display()
        ))),
        _ => Ok(keys),
    }
}

/// Whether a HID report descriptor has the FIDO usage page's CTAPHID usage.
fn is_fido(descriptor: &[u8]) -> bool {
    let mut page = 0;
    let mut at = 0;
    while at < descriptor.len() {
        let prefix = descriptor[at];
        if prefix == 0xFE {
            let Some(&size) = descriptor.get(at + 1) else { return false };
            at += 3 + size as usize;
            continue;
        }
        let size = match prefix & 0x03 {
            3 => 4,
            n => n as usize,
        };
        let Some(data) = descriptor.get(at + 1..at + 1 + size) else { return false };
        let value = data.iter().rev().fold(0u32, |n, b| n << 8 | *b as u32);
        match prefix & 0xFC {
            0x04 => page = value,
            0x08 if size == 4 => {
                if value >> 16 == FIDO_USAGE_PAGE && value & 0xFFFF == FIDO_USAGE_CTAPHID {
                    return true;
                }
            },
            0x08 if page == FIDO_USAGE_PAGE && value == FIDO_USAGE_CTAPHID => return true,
            _ => {},
        }
        at += 1 + size;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_a_security_key() {
        let yubikey = [
            0x06, 0xD0, 0xF1, 0x09, 0x01, 0xA1, 0x01, 0x09, 0x20, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x40,
            0x81, 0x02, 0x09, 0x21, 0x15, 0x00, 0x26, 0xFF, 0x00, 0x75, 0x08, 0x95, 0x40, 0x91, 0x02, 0xC0,
        ];
        assert!(is_fido(&yubikey));
    }

    #[test]
    fn ignores_other_devices() {
        let mouse = [0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x09, 0x01, 0xA1, 0x00, 0xC0, 0xC0];
        assert!(!is_fido(&mouse));
        assert!(!is_fido(&[0x06, 0xD0]));
    }
}
