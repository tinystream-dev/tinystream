// SPDX-License-Identifier: AGPL-3.0-or-later

//! CTAP2 over USB HID: the CTAPHID framing, and the commands a passkey ceremony needs.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use ring::rand::{SecureRandom, SystemRandom};

use super::cbor::{self, Value};
use super::pin::{self, Protocol};
use super::{Assertion, Attestation, Authenticator, Error, GetAssertion, MakeCredential, Prompt};

/// A USB HID device speaking CTAPHID, in 64-byte reports.
pub trait Hid: Send {
    fn write(&mut self, report: &[u8; 64]) -> io::Result<()>;

    /// The next report, or `None` when none came within `timeout`.
    fn read(&mut self, timeout: Duration) -> io::Result<Option<[u8; 64]>>;
}

/// Finds and opens the security keys plugged in now.
pub type Find = fn() -> Result<Vec<Box<dyn Hid>>, Error>;

const INIT: u8 = 0x06;
const CBOR: u8 = 0x10;
const CANCEL: u8 = 0x11;
const KEEPALIVE: u8 = 0x3B;
const ERROR: u8 = 0x3F;
const BROADCAST: [u8; 4] = [0xFF; 4];
const CAPABILITY_CBOR: u8 = 0x04;
const KEEPALIVE_UP_NEEDED: u8 = 2;

const MAKE_CREDENTIAL: u8 = 0x01;
const GET_ASSERTION: u8 = 0x02;
const GET_INFO: u8 = 0x04;
const CLIENT_PIN: u8 = 0x06;
const SELECTION: u8 = 0x0B;

const ERR_CREDENTIAL_EXCLUDED: u8 = 0x19;
const ERR_UNSUPPORTED_ALGORITHM: u8 = 0x26;
const ERR_OPERATION_DENIED: u8 = 0x27;
const ERR_KEY_STORE_FULL: u8 = 0x28;
const ERR_KEEPALIVE_CANCEL: u8 = 0x2D;
const ERR_NO_CREDENTIALS: u8 = 0x2E;
const ERR_USER_ACTION_TIMEOUT: u8 = 0x2F;
const ERR_PIN_INVALID: u8 = 0x31;
const ERR_PIN_BLOCKED: u8 = 0x32;
const ERR_PIN_AUTH_INVALID: u8 = 0x33;
const ERR_PIN_AUTH_BLOCKED: u8 = 0x34;
const ERR_PIN_NOT_SET: u8 = 0x35;
const ERR_PIN_REQUIRED: u8 = 0x36;

/// How long a key may stay quiet while it works on a command; it sends keepalives every 100 ms.
const SILENCE: Duration = Duration::from_secs(5);

/// What can go wrong while talking to one key.
#[derive(Debug)]
enum Failure {
    /// The key answered with this CTAP2 status.
    Status(u8),
    Cancelled,
    Gone,
    Garbled,
}

impl From<io::Error> for Failure {
    fn from(_: io::Error) -> Self {
        Self::Gone
    }
}

impl From<cbor::Malformed> for Failure {
    fn from(_: cbor::Malformed) -> Self {
        Self::Garbled
    }
}

impl From<pin::Failed> for Failure {
    fn from(_: pin::Failed) -> Self {
        Self::Garbled
    }
}

/// One key, on a channel of its own.
struct Device {
    hid: Box<dyn Hid>,
    channel: [u8; 4],
    cbor: bool,
}

impl Device {
    fn open(mut hid: Box<dyn Hid>) -> Result<Self, Failure> {
        let mut nonce = [0; 8];
        SystemRandom::new().fill(&mut nonce).map_err(|_| Failure::Garbled)?;
        write_message(hid.as_mut(), BROADCAST, INIT, &nonce)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Some(report) = hid.read(left)? else { break };
            if report[..4] == BROADCAST && report[4] == 0x80 | INIT && report[7..15] == nonce {
                let channel = report[15..19].try_into().map_err(|_| Failure::Garbled)?;
                return Ok(Self { hid, channel, cbor: report[23] & CAPABILITY_CBOR != 0 });
            }
        }
        Err(Failure::Gone)
    }

    /// Sends a CTAP2 command and waits for its answer, telling `touch` when the key wants a touch.
    fn command(
        &mut self,
        command: u8,
        parameters: Option<Value>,
        cancel: &AtomicBool,
        touch: &dyn Fn(),
    ) -> Result<Value, Failure> {
        let mut payload = vec![command];
        if let Some(parameters) = parameters {
            payload.extend(parameters.encode());
        }
        write_message(self.hid.as_mut(), self.channel, CBOR, &payload)?;
        let mut cancelled_at = None;
        loop {
            if cancel.load(Ordering::Relaxed) && cancelled_at.is_none() {
                write_message(self.hid.as_mut(), self.channel, CANCEL, &[])?;
                cancelled_at = Some(Instant::now());
            }
            if cancelled_at.is_some_and(|at| at.elapsed() > Duration::from_secs(1)) {
                return Err(Failure::Cancelled);
            }
            let (kind, data) = self.receive()?;
            match kind {
                KEEPALIVE => {
                    if data.first() == Some(&KEEPALIVE_UP_NEEDED) {
                        touch();
                    }
                },
                ERROR => return Err(Failure::Garbled),
                CBOR => {
                    let (&status, rest) = data.split_first().ok_or(Failure::Garbled)?;
                    return match status {
                        0 if rest.is_empty() => Ok(Value::Map(vec![])),
                        0 => Ok(Value::decode(rest)?),
                        ERR_KEEPALIVE_CANCEL if cancelled_at.is_some() => Err(Failure::Cancelled),
                        status => Err(Failure::Status(status)),
                    };
                },
                _ => return Err(Failure::Garbled),
            }
        }
    }

    /// The next whole message on our channel: its command and its data.
    fn receive(&mut self) -> Result<(u8, Vec<u8>), Failure> {
        let first = loop {
            let report = self.hid.read(SILENCE)?.ok_or(Failure::Gone)?;
            if report[..4] == self.channel && report[4] & 0x80 != 0 {
                break report;
            }
        };
        let length = u16::from_be_bytes([first[5], first[6]]) as usize;
        let mut data = first[7..].to_vec();
        let mut sequence = 0;
        while data.len() < length {
            let report = self.hid.read(SILENCE)?.ok_or(Failure::Gone)?;
            if report[..4] != self.channel {
                continue;
            }
            if report[4] != sequence {
                return Err(Failure::Garbled);
            }
            sequence += 1;
            data.extend_from_slice(&report[5..]);
        }
        data.truncate(length);
        Ok((first[4] & 0x7F, data))
    }
}

fn write_message(hid: &mut dyn Hid, channel: [u8; 4], command: u8, data: &[u8]) -> io::Result<()> {
    if data.len() > 57 + 128 * 59 {
        return Err(io::Error::other("message too long for CTAPHID"));
    }
    let mut report = [0; 64];
    report[..4].copy_from_slice(&channel);
    report[4] = 0x80 | command;
    report[5..7].copy_from_slice(&(data.len() as u16).to_be_bytes());
    let (first, rest) = data.split_at(data.len().min(57));
    report[7..7 + first.len()].copy_from_slice(first);
    hid.write(&report)?;
    for (sequence, chunk) in rest.chunks(59).enumerate() {
        let mut report = [0; 64];
        report[..4].copy_from_slice(&channel);
        report[4] = sequence as u8;
        report[5..5 + chunk.len()].copy_from_slice(chunk);
        hid.write(&report)?;
    }
    Ok(())
}

/// What the key says about itself (`authenticatorGetInfo`).
struct Info {
    options: Value,
    protocols: Vec<Protocol>,
}

impl Info {
    fn option(&self, name: &str) -> Option<bool> {
        self.options.get(name).and_then(Value::as_bool)
    }

    fn pin_set(&self) -> bool {
        self.option("clientPin") == Some(true)
    }

    fn protocol(&self) -> Protocol {
        self.protocols.first().copied().unwrap_or(Protocol::One)
    }
}

/// A ceremony with one key, which can ask for its PIN.
struct Session<'a> {
    device: Device,
    info: Info,
    prompt: &'a dyn Prompt,
    cancel: &'a AtomicBool,
}

impl Session<'_> {
    fn command(&mut self, command: u8, parameters: Option<Value>) -> Result<Value, Failure> {
        let prompt = self.prompt;
        self.device.command(command, parameters, self.cancel, &|| prompt.touch())
    }

    /// The key's PIN token for this ceremony, asking for the PIN until the key takes it.
    fn token(&mut self) -> Result<(Protocol, Vec<u8>), Error> {
        let protocol = self.info.protocol();
        let mut retries = None;
        let mut wrong = false;
        loop {
            if retries.is_none() {
                retries = self.retries(protocol).ok();
            }
            let Some(pin) = self.prompt.pin(retries, wrong) else { return Err(Error::Cancelled) };
            match self.pin_token(protocol, &pin) {
                Ok(token) => return Ok((protocol, token)),
                Err(Failure::Status(ERR_PIN_INVALID)) => {
                    wrong = true;
                    retries = None;
                },
                Err(failure) => return Err(explain(failure)),
            }
        }
    }

    fn retries(&mut self, protocol: Protocol) -> Result<u32, Failure> {
        let request = cbor::map([(1, protocol as i64), (2, 1)]);
        let response = self.command(CLIENT_PIN, Some(request))?;
        response.get(3).and_then(Value::as_int).map(|n| n as u32).ok_or(Failure::Garbled)
    }

    fn pin_token(&mut self, protocol: Protocol, pin: &str) -> Result<Vec<u8>, Failure> {
        let request = cbor::map([(1, protocol as i64), (2, 2)]);
        let response = self.command(CLIENT_PIN, Some(request))?;
        let peer = response.get(1).ok_or(Failure::Garbled)?;
        let (ours, shared) = pin::agree(protocol, peer)?;
        let request = cbor::map([
            (1, Value::Int(protocol as i64)),
            (2, Value::Int(5)),
            (3, ours),
            (6, Value::Bytes(shared.encrypt(&pin::pin_hash(pin)))),
        ]);
        let response = self.command(CLIENT_PIN, Some(request))?;
        let sealed = response.get(2).and_then(Value::as_bytes).ok_or(Failure::Garbled)?;
        Ok(shared.decrypt(sealed)?)
    }

    /// Runs `operation`, with a PIN token when the key wants one before or after trying without.
    fn with_pin(
        &mut self,
        needs_pin: bool,
        mut operation: impl FnMut(&mut Self, Option<&(Protocol, Vec<u8>)>) -> Result<Value, Failure>,
    ) -> Result<Value, Error> {
        let mut token = if needs_pin { Some(self.token()?) } else { None };
        loop {
            match operation(self, token.as_ref()) {
                Err(Failure::Status(ERR_PIN_REQUIRED | ERR_PIN_AUTH_INVALID))
                    if token.is_none() && self.info.pin_set() =>
                {
                    token = Some(self.token()?);
                },
                result => return result.map_err(explain),
            }
        }
    }
}

/// Adds `pinUvAuthParam` at `at` and `pinUvAuthProtocol` after it: 8 for `makeCredential`, 6 for
/// `getAssertion`.
fn pin_parameters(map: &mut Vec<(Value, Value)>, at: i64, token: Option<&(Protocol, Vec<u8>)>, hash: &[u8]) {
    if let Some((protocol, token)) = token {
        map.push((Value::Int(at), Value::Bytes(pin::authenticate(*protocol, token, hash))));
        map.push((Value::Int(at + 1), Value::Int(*protocol as i64)));
    }
}

fn credential_list(ids: &[Vec<u8>]) -> Value {
    Value::Array(
        ids.iter()
            .map(|id| cbor::map([("type", Value::from("public-key")), ("id", Value::Bytes(id.clone()))]))
            .collect(),
    )
}

/// What the person sees for a key's answer.
fn explain(failure: Failure) -> Error {
    let message = match failure {
        Failure::Cancelled => return Error::Cancelled,
        Failure::Gone => "The security key stopped answering. Plug it back in and try again.",
        Failure::Garbled => "The security key sent something the app doesn't understand.",
        Failure::Status(status) => match status {
            ERR_CREDENTIAL_EXCLUDED => "This security key is already one of your passkeys.",
            ERR_NO_CREDENTIALS => "This security key isn't one of your passkeys on this server.",
            ERR_OPERATION_DENIED => "The security key turned the request down.",
            ERR_USER_ACTION_TIMEOUT => "The security key wasn't touched in time. Try again.",
            ERR_UNSUPPORTED_ALGORITHM => "This security key can't make the kind of passkey the server asks for.",
            ERR_KEY_STORE_FULL => "This security key has no room for another passkey.",
            ERR_PIN_BLOCKED => {
                "This security key's PIN is blocked after too many wrong tries. Reset the key to use it again."
            },
            ERR_PIN_AUTH_BLOCKED => {
                "Too many wrong PINs in a row. Unplug the security key, plug it back in and try again."
            },
            ERR_PIN_NOT_SET | ERR_PIN_REQUIRED => {
                "This security key needs a PIN, but none is set. Set one in your key's app first."
            },
            status => return Error::Failed(format!("The security key reported error 0x{status:02x}.")),
        },
    };
    Error::Failed(message.into())
}

/// Security keys reached over CTAP2, found by `find`.
pub struct Ctap2 {
    find: Find,
}

impl Ctap2 {
    pub fn new(find: Find) -> Self {
        Self { find }
    }

    /// Waits for a key, picking the one that's touched when there are several.
    fn session<'a>(&self, prompt: &'a dyn Prompt, timeout: Duration) -> Result<Session<'a>, Error> {
        let cancel = prompt.cancel();
        let deadline = Instant::now() + timeout;
        let mut asked = false;
        let devices = loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            let devices: Vec<Device> = (self.find)()?.into_iter().filter_map(|hid| Device::open(hid).ok()).collect();
            if !devices.is_empty() {
                if asked {
                    prompt.connected();
                }
                break devices;
            }
            if Instant::now() > deadline {
                return Err(Error::Failed("No security key was plugged in. Try again.".into()));
            }
            if !asked {
                prompt.insert();
                asked = true;
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        let mut device = if devices.len() == 1 {
            devices.into_iter().next().ok_or(Error::Cancelled)?
        } else {
            prompt.touch();
            select(devices, cancel)?
        };
        if !device.cbor {
            return Err(Error::Failed(
                "This security key only speaks the older U2F protocol, which the app doesn't support.".into(),
            ));
        }
        let info = device.command(GET_INFO, None, cancel, &|| {}).map_err(explain)?;
        let info = Info {
            options: info.get(4).cloned().unwrap_or(Value::Null),
            protocols: info
                .get(6)
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(|v| match v.as_int() {
                    Some(1) => Some(Protocol::One),
                    Some(2) => Some(Protocol::Two),
                    _ => None,
                })
                .collect(),
        };
        Ok(Session { device, info, prompt, cancel })
    }
}

/// The key the person touches, out of several; the others are told to stop waiting.
fn select(devices: Vec<Device>, cancel: &AtomicBool) -> Result<Device, Error> {
    let stop = Arc::new(AtomicBool::new(false));
    let (send, receive) = mpsc::channel();
    let threads: Vec<_> = devices
        .into_iter()
        .map(|mut device| {
            let (stop, send) = (stop.clone(), send.clone());
            std::thread::spawn(move || {
                let touched = touch_to_select(&mut device, &stop);
                let _ = send.send(touched.map(|_| device));
            })
        })
        .collect();
    drop(send);
    let mut chosen = None;
    let mut failure = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            stop.store(true, Ordering::Relaxed);
        }
        match receive.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(device)) if chosen.is_none() => {
                chosen = Some(device);
                stop.store(true, Ordering::Relaxed);
            },
            Ok(Ok(_)) => {},
            Ok(Err(f)) => failure = failure.or(Some(f)),
            Err(mpsc::RecvTimeoutError::Timeout) => {},
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    for thread in threads {
        let _ = thread.join();
    }
    chosen.ok_or_else(|| explain(failure.unwrap_or(Failure::Cancelled)))
}

/// Waits for a touch: `authenticatorSelection` on CTAP 2.1 keys, and the empty-PIN
/// `makeCredential` browsers use on older ones.
fn touch_to_select(device: &mut Device, stop: &AtomicBool) -> Result<(), Failure> {
    if !device.cbor {
        return Err(Failure::Garbled);
    }
    let info = device.command(GET_INFO, None, stop, &|| {})?;
    let modern =
        info.get(1).and_then(Value::as_array).is_some_and(|v| v.iter().any(|v| v.as_text() == Some("FIDO_2_1")));
    if modern {
        return device.command(SELECTION, None, stop, &|| {}).map(|_| ());
    }
    let request = cbor::map([
        (1, Value::Bytes(vec![0; 32])),
        (2, cbor::map([("id", ".dummy")])),
        (3, cbor::map([("id", Value::Bytes(vec![1])), ("name", Value::from("dummy"))])),
        (4, Value::Array(vec![cbor::map([("alg", Value::Int(-7)), ("type", Value::from("public-key"))])])),
        (8, Value::Bytes(vec![])),
        (9, Value::Int(1)),
    ]);
    match device.command(MAKE_CREDENTIAL, Some(request), stop, &|| {}) {
        Ok(_) | Err(Failure::Status(ERR_PIN_INVALID | ERR_PIN_NOT_SET | ERR_PIN_AUTH_INVALID)) => Ok(()),
        Err(failure) => Err(failure),
    }
}

impl Authenticator for Ctap2 {
    fn make_credential(&self, request: &MakeCredential, prompt: &dyn Prompt) -> Result<Attestation, Error> {
        let mut session = self.session(prompt, request.timeout)?;
        let needs_pin = session.info.pin_set()
            && (session.info.option("makeCredUvNotRqd") != Some(true) || session.info.option("alwaysUv") == Some(true));
        let response = session.with_pin(needs_pin, |session, token| {
            let mut parameters = vec![
                (Value::Int(1), Value::Bytes(request.client_data_hash.to_vec())),
                (Value::Int(2), cbor::map([("id", request.rp_id.as_str()), ("name", request.rp_name.as_str())])),
                (
                    Value::Int(3),
                    cbor::map([
                        ("id", Value::Bytes(request.user_id.clone())),
                        ("name", Value::from(request.user_name.as_str())),
                        ("displayName", Value::from(request.user_display_name.as_str())),
                    ]),
                ),
                (
                    Value::Int(4),
                    Value::Array(
                        request
                            .algorithms
                            .iter()
                            .map(|alg| cbor::map([("alg", Value::Int(*alg)), ("type", Value::from("public-key"))]))
                            .collect(),
                    ),
                ),
            ];
            if !request.exclude.is_empty() {
                parameters.push((Value::Int(5), credential_list(&request.exclude)));
            }
            pin_parameters(&mut parameters, 8, token, &request.client_data_hash);
            session.command(MAKE_CREDENTIAL, Some(Value::Map(parameters)))
        })?;
        let auth_data = response.get(2).and_then(Value::as_bytes).ok_or_else(|| explain(Failure::Garbled))?.to_vec();
        let credential_id = attested_credential_id(&auth_data).ok_or_else(|| explain(Failure::Garbled))?;
        let attestation_object = cbor::map([
            ("fmt", Value::from("none")),
            ("attStmt", Value::Map(vec![])),
            ("authData", Value::Bytes(auth_data)),
        ])
        .encode();
        Ok(Attestation { attestation_object, credential_id })
    }

    fn get_assertion(&self, request: &GetAssertion, prompt: &dyn Prompt) -> Result<Assertion, Error> {
        let mut session = self.session(prompt, request.timeout)?;
        let needs_pin = session.info.pin_set() && session.info.option("alwaysUv") == Some(true);
        let response = session.with_pin(needs_pin, |session, token| {
            let mut parameters = vec![
                (Value::Int(1), Value::from(request.rp_id.as_str())),
                (Value::Int(2), Value::Bytes(request.client_data_hash.to_vec())),
            ];
            if !request.allow.is_empty() {
                parameters.push((Value::Int(3), credential_list(&request.allow)));
            }
            pin_parameters(&mut parameters, 6, token, &request.client_data_hash);
            session.command(GET_ASSERTION, Some(Value::Map(parameters)))
        })?;
        let garbled = || explain(Failure::Garbled);
        let credential_id = match response.get(1).and_then(|c| c.get("id")).and_then(Value::as_bytes) {
            Some(id) => id.to_vec(),
            None if request.allow.len() == 1 => request.allow[0].clone(),
            None => return Err(garbled()),
        };
        Ok(Assertion {
            credential_id,
            authenticator_data: response.get(2).and_then(Value::as_bytes).ok_or_else(garbled)?.to_vec(),
            signature: response.get(3).and_then(Value::as_bytes).ok_or_else(garbled)?.to_vec(),
            user_handle: response.get(4).and_then(|u| u.get("id")).and_then(Value::as_bytes).map(<[u8]>::to_vec),
        })
    }
}

/// The credential ID in authenticator data with attested credential data.
fn attested_credential_id(auth_data: &[u8]) -> Option<Vec<u8>> {
    const AT: u8 = 0x40;
    if auth_data.len() < 55 || auth_data[32] & AT == 0 {
        return None;
    }
    let length = u16::from_be_bytes([auth_data[53], auth_data[54]]) as usize;
    auth_data.get(55..55 + length).map(<[u8]>::to_vec)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    /// A key that answers each command from a script, after the channel is set up.
    struct Scripted {
        channel: [u8; 4],
        sent: Arc<Mutex<Vec<[u8; 64]>>>,
        replies: VecDeque<Vec<(u8, Vec<u8>)>>,
        pending: VecDeque<[u8; 64]>,
    }

    impl Scripted {
        fn new(replies: Vec<Vec<(u8, Vec<u8>)>>) -> (Self, Arc<Mutex<Vec<[u8; 64]>>>) {
            let sent = Arc::new(Mutex::new(vec![]));
            let key =
                Self { channel: [1, 2, 3, 4], sent: sent.clone(), replies: replies.into(), pending: VecDeque::new() };
            (key, sent)
        }

        fn queue(&mut self, channel: [u8; 4], command: u8, data: &[u8]) {
            let mut sink = Sink(vec![]);
            write_message(&mut sink, channel, command, data).unwrap();
            self.pending.extend(sink.0);
        }
    }

    struct Sink(Vec<[u8; 64]>);

    impl Hid for Sink {
        fn write(&mut self, report: &[u8; 64]) -> io::Result<()> {
            self.0.push(*report);
            Ok(())
        }

        fn read(&mut self, _: Duration) -> io::Result<Option<[u8; 64]>> {
            Ok(None)
        }
    }

    impl Hid for Scripted {
        fn write(&mut self, report: &[u8; 64]) -> io::Result<()> {
            self.sent.lock().unwrap().push(*report);
            if report[..4] == BROADCAST && report[4] == 0x80 | INIT {
                let mut reply = report[7..15].to_vec();
                reply.extend_from_slice(&self.channel);
                reply.extend_from_slice(&[2, 5, 2, 7, CAPABILITY_CBOR]);
                self.queue(BROADCAST, INIT, &reply);
            } else if report[4] & 0x80 != 0 && report[4] != 0x80 | CANCEL {
                self.queue([9, 9, 9, 9], CBOR, &[ERR_NO_CREDENTIALS]);
                for (command, data) in self.replies.pop_front().unwrap_or_default() {
                    self.queue(self.channel, command, &data);
                }
            }
            Ok(())
        }

        fn read(&mut self, _: Duration) -> io::Result<Option<[u8; 64]>> {
            Ok(self.pending.pop_front())
        }
    }

    #[test]
    fn frames_long_messages_in_sequence() {
        let mut sink = Sink(vec![]);
        let data: Vec<u8> = (0..200).map(|n| n as u8).collect();
        write_message(&mut sink, [1, 2, 3, 4], CBOR, &data).unwrap();
        assert_eq!(sink.0.len(), 4);
        assert_eq!(sink.0[0][4], 0x90);
        assert_eq!(sink.0[0][5..7], [0, 200]);
        assert_eq!(sink.0[1][4], 0);
        assert_eq!(sink.0[3][4], 2);
        let mut back = sink.0[0][7..].to_vec();
        for report in &sink.0[1..] {
            back.extend_from_slice(&report[5..]);
        }
        assert_eq!(&back[..200], &data[..]);
    }

    #[test]
    fn reassembles_answers_and_skips_other_channels() {
        let body = cbor::map([(1, Value::Bytes(vec![7; 150]))]);
        let mut answer = vec![0];
        answer.extend(body.encode());
        let (key, sent) = Scripted::new(vec![vec![(KEEPALIVE, vec![KEEPALIVE_UP_NEEDED]), (CBOR, answer)]]);
        let mut device = Device::open(Box::new(key)).unwrap();
        assert_eq!(device.channel, [1, 2, 3, 4]);
        assert!(device.cbor);
        let touched = AtomicBool::new(false);
        let result =
            device.command(GET_INFO, None, &AtomicBool::new(false), &|| touched.store(true, Ordering::Relaxed));
        assert_eq!(result.unwrap(), body);
        assert!(touched.load(Ordering::Relaxed));
        assert_eq!(sent.lock().unwrap()[1][..8], [1, 2, 3, 4, 0x90, 0, 1, GET_INFO]);
    }

    #[test]
    fn reports_a_keys_status() {
        let (key, _) = Scripted::new(vec![vec![(CBOR, vec![ERR_NO_CREDENTIALS])]]);
        let mut device = Device::open(Box::new(key)).unwrap();
        let result = device.command(GET_ASSERTION, None, &AtomicBool::new(false), &|| {});
        assert!(matches!(result, Err(Failure::Status(ERR_NO_CREDENTIALS))));
    }

    #[test]
    fn puts_the_pin_proof_where_each_command_reads_it() {
        let token = (Protocol::Two, vec![5; 32]);
        let mut assertion = vec![];
        pin_parameters(&mut assertion, 6, Some(&token), &[0; 32]);
        assert_eq!(assertion[0].0, Value::Int(6));
        assert_eq!(assertion[1], (Value::Int(7), Value::Int(2)));
        let mut none = vec![];
        pin_parameters(&mut none, 8, None, &[0; 32]);
        assert!(none.is_empty());
    }

    #[test]
    fn finds_the_attested_credential() {
        let mut data = vec![0; 32];
        data.push(0x41);
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.extend_from_slice(&[0; 16]);
        data.extend_from_slice(&[0, 3, 9, 8, 7]);
        data.extend_from_slice(&[0xA0]);
        assert_eq!(attested_credential_id(&data), Some(vec![9, 8, 7]));
        data[32] = 0x01;
        assert_eq!(attested_credential_id(&data), None);
    }
}
