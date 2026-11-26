// SPDX-License-Identifier: AGPL-3.0-or-later

//! Passkeys on security keys (#95): what a browser does for `navigator.credentials`, from the
//! server's WebAuthn options to the credential it checks. The server's relying party comes from
//! the request's `Origin`, so the app speaks for the server's own page.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::digest;
use serde_json::{Value, json};
use url::Url;

use crate::api;
use crate::platform::{self, Desktop};

pub mod cbor;
mod ctap;
mod pin;
pub mod view;

pub use ctap::{Ctap2, Hid};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The person closed the prompt.
    Cancelled,
    Failed(String),
}

impl From<api::Error> for Error {
    fn from(error: api::Error) -> Self {
        Self::Failed(error.message)
    }
}

/// `authenticatorMakeCredential`, as the server asked for it.
pub struct MakeCredential {
    pub client_data_hash: [u8; 32],
    pub rp_id: String,
    pub rp_name: String,
    pub user_id: Vec<u8>,
    pub user_name: String,
    pub user_display_name: String,
    pub algorithms: Vec<i64>,
    pub exclude: Vec<Vec<u8>>,
    pub timeout: Duration,
}

pub struct Attestation {
    pub attestation_object: Vec<u8>,
    pub credential_id: Vec<u8>,
}

/// `authenticatorGetAssertion`, as the server asked for it.
pub struct GetAssertion {
    pub client_data_hash: [u8; 32],
    pub rp_id: String,
    pub allow: Vec<Vec<u8>>,
    pub timeout: Duration,
}

pub struct Assertion {
    pub credential_id: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub signature: Vec<u8>,
    pub user_handle: Option<Vec<u8>>,
}

/// Whatever reaches the security key: CTAP2 over HID here, `webauthn.dll` on Windows and
/// AuthenticationServices on macOS later. Both calls block until the key answers.
pub trait Authenticator: Send + Sync {
    fn make_credential(&self, request: &MakeCredential, prompt: &dyn Prompt) -> Result<Attestation, Error>;

    fn get_assertion(&self, request: &GetAssertion, prompt: &dyn Prompt) -> Result<Assertion, Error>;
}

/// What the key needs from the person while a ceremony runs.
pub trait Prompt: Sync {
    /// Set once the person closes the prompt.
    fn cancel(&self) -> &AtomicBool;

    /// No key is plugged in yet.
    fn insert(&self);

    /// A key was plugged in after [`Prompt::insert`].
    fn connected(&self);

    fn touch(&self);

    /// Asks for the key's PIN; `None` when the person closed the prompt instead.
    fn pin(&self, retries: Option<u32>, wrong: bool) -> Option<String>;
}

/// What the prompt shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Waiting,
    Insert,
    Touch,
    Pin { retries: Option<u32>, wrong: bool },
}

/// Where a typed PIN goes back to the key.
#[derive(Clone)]
pub struct Reply(mpsc::Sender<Option<String>>);

impl fmt::Debug for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Reply")
    }
}

#[derive(Debug, Clone)]
pub struct Event(Stage, Option<Reply>);

/// A running ceremony, as the screen that started it holds it. Dropping it closes the prompt.
pub struct Ceremony {
    pub stage: Stage,
    pub pin: String,
    reply: Option<Reply>,
    cancel: Arc<AtomicBool>,
}

impl Ceremony {
    /// A ceremony, and the bridge its key talks through.
    pub fn start() -> (Self, Bridge) {
        let cancel = Arc::new(AtomicBool::new(false));
        let bridge =
            Bridge { cancel: cancel.clone(), send: Arc::new(|_| {}), stage: Arc::new(Mutex::new(Stage::Waiting)) };
        (Self { stage: Stage::Waiting, pin: String::new(), reply: None, cancel }, bridge)
    }

    pub fn event(&mut self, Event(stage, reply): Event) {
        if reply.is_some() || !matches!(stage, Stage::Pin { .. }) {
            self.reply = reply;
        }
        self.stage = stage;
    }

    pub fn submit(&mut self) {
        if self.pin.is_empty() {
            return;
        }
        if let Some(Reply(reply)) = self.reply.take() {
            let _ = reply.send(Some(std::mem::take(&mut self.pin)));
            self.stage = Stage::Waiting;
        }
    }
}

impl Drop for Ceremony {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(Reply(reply)) = self.reply.take() {
            let _ = reply.send(None);
        }
    }
}

/// The key's side of a [`Ceremony`].
#[derive(Clone)]
pub struct Bridge {
    cancel: Arc<AtomicBool>,
    send: Arc<dyn Fn(Event) + Send + Sync>,
    stage: Arc<Mutex<Stage>>,
}

impl Bridge {
    /// Calls `send` for each change the screen should show.
    pub fn sending(self, send: impl Fn(Event) + Send + Sync + 'static) -> Self {
        Self { send: Arc::new(send), ..self }
    }

    fn show(&self, stage: Stage, reply: Option<Reply>) {
        let mut shown = self.stage.lock().unwrap_or_else(|e| e.into_inner());
        if *shown != stage || reply.is_some() {
            *shown = stage.clone();
            (self.send)(Event(stage, reply));
        }
    }
}

impl Prompt for Bridge {
    fn cancel(&self) -> &AtomicBool {
        &self.cancel
    }

    fn insert(&self) {
        self.show(Stage::Insert, None);
    }

    fn connected(&self) {
        self.show(Stage::Waiting, None);
    }

    fn touch(&self) {
        self.show(Stage::Touch, None);
    }

    fn pin(&self, retries: Option<u32>, wrong: bool) -> Option<String> {
        let (send, receive) = mpsc::channel();
        self.show(Stage::Pin { retries, wrong }, Some(Reply(send)));
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return None;
            }
            match receive.recv_timeout(Duration::from_millis(100)) {
                Ok(pin) => {
                    self.show(Stage::Waiting, None);
                    return pin;
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {},
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

/// Makes a passkey for the server's `startPasskeyRegistration` options, returning the credential
/// for `finishPasskeyRegistration`.
pub async fn create(options: Value, origin: Url, bridge: Bridge) -> Result<Value, Error> {
    let (request, client_data) = registration(&options, &origin)?;
    let attestation = run(move |authenticator| authenticator.make_credential(&request, &bridge)).await?;
    Ok(registered(&attestation, &client_data))
}

/// Signs in with a passkey for the server's `startPasskeySignIn` options, returning the
/// credential for `finishPasskeySignIn`.
pub async fn get(options: Value, origin: Url, bridge: Bridge) -> Result<Value, Error> {
    let (request, client_data) = sign_in(&options, &origin)?;
    let assertion = run(move |authenticator| authenticator.get_assertion(&request, &bridge)).await?;
    Ok(signed_in(&assertion, &client_data))
}

async fn run<T: Send + 'static>(
    ceremony: impl FnOnce(&dyn Authenticator) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let Some(authenticator) = platform::Current::authenticator() else {
        return Err(Error::Failed("Security keys aren't supported on this platform yet.".into()));
    };
    tokio::task::spawn_blocking(move || ceremony(authenticator.as_ref()))
        .await
        .unwrap_or_else(|_| Err(Error::Failed("The security key stopped answering.".into())))
}

fn invalid() -> Error {
    Error::Failed("The server sent passkey options the app doesn't understand.".into())
}

fn bytes(value: &Value) -> Result<Vec<u8>, Error> {
    let text = value.as_str().ok_or_else(invalid)?;
    let text: String = text
        .trim_end_matches('=')
        .chars()
        .map(|c| match c {
            '+' => '-',
            '/' => '_',
            c => c,
        })
        .collect();
    URL_SAFE_NO_PAD.decode(text).map_err(|_| invalid())
}

fn text(value: &Value) -> Result<String, Error> {
    value.as_str().map(str::to_owned).ok_or_else(invalid)
}

/// The IDs in a list of credential descriptors.
fn credentials(value: &Value) -> Result<Vec<Vec<u8>>, Error> {
    value
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|c| c["type"] == "public-key")
        .map(|c| bytes(&c["id"]))
        .collect()
}

fn timeout(options: &Value) -> Duration {
    Duration::from_millis(options["timeout"].as_u64().unwrap_or(120_000).clamp(30_000, 300_000))
}

/// The relying party the server named, which must be the origin's host or a domain above it, as
/// browsers require.
fn relying_party(rp_id: Option<&str>, origin: &Url) -> Result<String, Error> {
    let host = origin.host_str().ok_or_else(invalid)?;
    let rp_id = rp_id.unwrap_or(host);
    if host == rp_id || host.ends_with(&format!(".{rp_id}")) {
        Ok(rp_id.to_owned())
    } else {
        Err(Error::Failed("The server asked for a passkey for another site.".into()))
    }
}

/// The browser's `clientDataJSON`, for a page on `origin`.
fn client_data(kind: &str, challenge: &[u8], origin: &Url) -> Vec<u8> {
    let quote = |s: &str| Value::String(s.to_owned()).to_string();
    format!(
        r#"{{"type":{},"challenge":{},"origin":{},"crossOrigin":false}}"#,
        quote(kind),
        quote(&URL_SAFE_NO_PAD.encode(challenge)),
        quote(&origin.origin().ascii_serialization()),
    )
    .into_bytes()
}

fn hash(data: &[u8]) -> [u8; 32] {
    digest::digest(&digest::SHA256, data).as_ref().try_into().unwrap_or([0; 32])
}

fn registration(options: &Value, origin: &Url) -> Result<(MakeCredential, Vec<u8>), Error> {
    let options = &options["publicKey"];
    let client_data = client_data("webauthn.create", &bytes(&options["challenge"])?, origin);
    let user = &options["user"];
    let algorithms: Vec<i64> = options["pubKeyCredParams"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|p| p["type"] == "public-key")
        .filter_map(|p| p["alg"].as_i64())
        .collect();
    if algorithms.is_empty() {
        return Err(invalid());
    }
    let request = MakeCredential {
        client_data_hash: hash(&client_data),
        rp_id: relying_party(options["rp"]["id"].as_str(), origin)?,
        rp_name: options["rp"]["name"].as_str().unwrap_or("tinystream").to_owned(),
        user_id: bytes(&user["id"])?,
        user_name: text(&user["name"])?,
        user_display_name: user["displayName"].as_str().map_or_else(|| text(&user["name"]), |s| Ok(s.to_owned()))?,
        algorithms,
        exclude: credentials(&options["excludeCredentials"])?,
        timeout: timeout(options),
    };
    Ok((request, client_data))
}

fn sign_in(options: &Value, origin: &Url) -> Result<(GetAssertion, Vec<u8>), Error> {
    let options = &options["publicKey"];
    let client_data = client_data("webauthn.get", &bytes(&options["challenge"])?, origin);
    let request = GetAssertion {
        client_data_hash: hash(&client_data),
        rp_id: relying_party(options["rpId"].as_str(), origin)?,
        allow: credentials(&options["allowCredentials"])?,
        timeout: timeout(options),
    };
    Ok((request, client_data))
}

fn registered(attestation: &Attestation, client_data: &[u8]) -> Value {
    let id = URL_SAFE_NO_PAD.encode(&attestation.credential_id);
    json!({
        "id": id,
        "rawId": id,
        "type": "public-key",
        "extensions": {},
        "response": {
            "attestationObject": URL_SAFE_NO_PAD.encode(&attestation.attestation_object),
            "clientDataJSON": URL_SAFE_NO_PAD.encode(client_data),
            "transports": ["usb"],
        },
    })
}

fn signed_in(assertion: &Assertion, client_data: &[u8]) -> Value {
    let id = URL_SAFE_NO_PAD.encode(&assertion.credential_id);
    json!({
        "id": id,
        "rawId": id,
        "type": "public-key",
        "extensions": {},
        "response": {
            "authenticatorData": URL_SAFE_NO_PAD.encode(&assertion.authenticator_data),
            "clientDataJSON": URL_SAFE_NO_PAD.encode(client_data),
            "signature": URL_SAFE_NO_PAD.encode(&assertion.signature),
            "userHandle": assertion.user_handle.as_ref().map(|h| URL_SAFE_NO_PAD.encode(h)),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> Url {
        Url::parse("https://tv.example.com/").unwrap()
    }

    #[test]
    fn writes_client_data_like_a_browser() {
        let data = client_data("webauthn.get", &[1, 2, 255], &Url::parse("http://localhost:8080/app").unwrap());
        assert_eq!(
            String::from_utf8(data).unwrap(),
            r#"{"type":"webauthn.get","challenge":"AQL_","origin":"http://localhost:8080","crossOrigin":false}"#
        );
    }

    #[test]
    fn reads_the_servers_registration_options() {
        let options = json!({"publicKey": {
            "rp": {"name": "tinystream", "id": "tv.example.com"},
            "user": {"id": "AAEC", "name": "david", "displayName": "david"},
            "challenge": "q83v",
            "pubKeyCredParams": [{"type": "public-key", "alg": -7}, {"type": "public-key", "alg": -257}],
            "timeout": 60000,
            "excludeCredentials": [{"type": "public-key", "id": "BAUG"}],
            "authenticatorSelection": {"requireResidentKey": false, "userVerification": "discouraged"},
            "attestation": "none",
        }});
        let (request, client_data) = registration(&options, &origin()).unwrap();
        assert_eq!(request.rp_id, "tv.example.com");
        assert_eq!(request.user_id, [0, 1, 2]);
        assert_eq!(request.algorithms, [-7, -257]);
        assert_eq!(request.exclude, [vec![4, 5, 6]]);
        assert_eq!(request.timeout, Duration::from_secs(60));
        assert_eq!(request.client_data_hash, hash(&client_data));
        let client: Value = serde_json::from_slice(&client_data).unwrap();
        assert_eq!(client["challenge"], "q83v");
        assert_eq!(client["origin"], "https://tv.example.com");
    }

    #[test]
    fn reads_the_servers_sign_in_options() {
        let options = json!({"publicKey": {
            "challenge": "q83v",
            "rpId": "example.com",
            "allowCredentials": [{"type": "public-key", "id": "BAUG"}, {"type": "other", "id": "AAAA"}],
            "userVerification": "discouraged",
        }});
        let (request, _) = sign_in(&options, &origin()).unwrap();
        assert_eq!(request.rp_id, "example.com");
        assert_eq!(request.allow, [vec![4, 5, 6]]);
        assert_eq!(request.timeout, Duration::from_secs(120));
    }

    #[test]
    fn refuses_another_sites_relying_party() {
        assert!(relying_party(Some("evil.com"), &origin()).is_err());
        assert!(relying_party(Some("ample.com"), &origin()).is_err());
        assert_eq!(relying_party(None, &origin()).unwrap(), "tv.example.com");
    }

    #[test]
    fn writes_credentials_the_server_reads() {
        let attestation = Attestation { attestation_object: vec![0xA0], credential_id: vec![1, 2, 3] };
        let credential = registered(&attestation, b"{}");
        assert_eq!(credential["rawId"], "AQID");
        assert_eq!(credential["response"]["clientDataJSON"], "e30");
        let assertion = Assertion {
            credential_id: vec![1, 2, 3],
            authenticator_data: vec![9],
            signature: vec![8],
            user_handle: None,
        };
        let credential = signed_in(&assertion, b"{}");
        assert_eq!(credential["response"]["signature"], "CA");
        assert!(credential["response"]["userHandle"].is_null());
    }

    #[test]
    fn hands_a_typed_pin_to_the_key() {
        let (mut ceremony, bridge) = Ceremony::start();
        let events = Arc::new(Mutex::new(vec![]));
        let seen = events.clone();
        let bridge = bridge.sending(move |event| seen.lock().unwrap().push(event));
        let key = std::thread::spawn(move || bridge.pin(Some(8), false));
        let event = loop {
            if let Some(event) = events.lock().unwrap().pop() {
                break event;
            }
            std::thread::yield_now();
        };
        ceremony.event(event);
        assert_eq!(ceremony.stage, Stage::Pin { retries: Some(8), wrong: false });
        ceremony.pin = "1234".into();
        ceremony.submit();
        assert_eq!(key.join().unwrap().as_deref(), Some("1234"));
        assert!(ceremony.pin.is_empty());
    }

    #[test]
    fn closing_the_prompt_stops_the_key() {
        let (ceremony, bridge) = Ceremony::start();
        let key = std::thread::spawn(move || bridge.pin(None, false));
        drop(ceremony);
        assert_eq!(key.join().unwrap(), None);
    }
}
