// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, header};
use rand::RngCore;
use url::Url;
use webauthn_rs::prelude::*;

use crate::error::{ApiError, ApiResult};

enum Pending {
    Register { user_id: i64, name: String, state: SecurityKeyRegistration },
    Login { user_id: i64, state: SecurityKeyAuthentication },
}

#[derive(Default)]
pub struct Challenges(Mutex<HashMap<String, (Instant, Pending)>>);

const TTL: Duration = Duration::from_secs(300);

impl Challenges {
    fn put(&self, pending: Pending) -> String {
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        let id = hex::encode(bytes);
        let mut map = self.0.lock().unwrap();
        map.retain(|_, (t, _)| t.elapsed() < TTL);
        map.insert(id.clone(), (Instant::now(), pending));
        id
    }

    fn take(&self, id: &str) -> Option<Pending> {
        let (t, p) = self.0.lock().unwrap().remove(id)?;
        (t.elapsed() < TTL).then_some(p)
    }
}

pub fn webauthn_for(headers: &HeaderMap) -> ApiResult<Webauthn> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .and_then(|o| Url::parse(o).ok())
        .ok_or_else(|| ApiError::bad_request("passkeys need a browser request with an Origin"))?;

    let rp_id = origin.host_str().ok_or_else(|| ApiError::bad_request("the page's origin has no host"))?.to_string();

    WebauthnBuilder::new(&rp_id, &origin)
        .and_then(|b| b.rp_name("tinystream").danger_set_user_presence_only_security_keys(true).build())
        .map_err(|e| ApiError::bad_request(format!("passkeys need a secure page (https, or http://localhost): {e}")))
}

fn webauthn_err(e: WebauthnError) -> ApiError {
    ApiError::bad_request(format!("the passkey didn't check out: {e}"))
}

pub fn start_registration(
    challenges: &Challenges,
    wa: &Webauthn,
    user_id: i64,
    handle: Uuid,
    username: &str,
    name: String,
    existing: Vec<SecurityKey>,
) -> ApiResult<(String, CreationChallengeResponse)> {
    let exclude = existing.iter().map(|k| k.cred_id().clone()).collect();

    let (ccr, state) = wa
        .start_securitykey_registration(handle, username, username, Some(exclude), None, None)
        .map_err(webauthn_err)?;

    let id = challenges.put(Pending::Register { user_id, name, state });
    Ok((id, ccr))
}

pub fn finish_registration(
    challenges: &Challenges,
    wa: &Webauthn,
    user_id: i64,
    challenge: &str,
    credential: &RegisterPublicKeyCredential,
) -> ApiResult<(String, SecurityKey)> {
    match challenges.take(challenge) {
        Some(Pending::Register { user_id: u, name, state }) if u == user_id => {
            let key = wa.finish_securitykey_registration(credential, &state).map_err(webauthn_err)?;
            Ok((name, key))
        },
        _ => Err(ApiError::bad_request("that took too long; try again")),
    }
}

pub fn start_login(
    challenges: &Challenges,
    wa: &Webauthn,
    user_id: i64,
    keys: &[SecurityKey],
) -> ApiResult<(String, RequestChallengeResponse)> {
    let (rcr, state) = wa.start_securitykey_authentication(keys).map_err(webauthn_err)?;
    let id = challenges.put(Pending::Login { user_id, state });
    Ok((id, rcr))
}

pub fn finish_login(
    challenges: &Challenges,
    wa: &Webauthn,
    challenge: &str,
    credential: &PublicKeyCredential,
) -> ApiResult<(i64, AuthenticationResult)> {
    match challenges.take(challenge) {
        Some(Pending::Login { user_id, state }) => {
            let result = wa.finish_securitykey_authentication(credential, &state).map_err(webauthn_err)?;
            Ok((user_id, result))
        },
        _ => Err(ApiError::bad_request("that took too long; try again")),
    }
}
