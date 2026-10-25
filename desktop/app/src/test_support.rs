// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures::future::BoxFuture;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

use crate::api::Token;
use crate::secrets::{Secrets, Stored};

pub struct Directory(pub PathBuf);

impl Directory {
    pub fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "tinystream-{name}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
pub struct MemorySecrets(Mutex<HashMap<u64, Token>>);

impl Secrets for MemorySecrets {
    fn get(&self, id: u64) -> BoxFuture<'_, Result<Stored>> {
        Box::pin(async move { Ok(Stored { token: self.0.lock().unwrap().get(&id).cloned(), fallback: false }) })
    }

    fn set(&self, id: u64, token: Option<Token>) -> BoxFuture<'_, Result<bool>> {
        Box::pin(async move {
            let mut tokens = self.0.lock().unwrap();
            if let Some(token) = token {
                tokens.insert(id, token);
            } else {
                tokens.remove(&id);
            }
            Ok(false)
        })
    }
}

pub fn run<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
}

/// A real HTTP boundary, without another server framework in the desktop dependency graph.
pub async fn http(
    count: usize,
    respond: impl Fn(&Value) -> Value + Send + 'static,
) -> (Url, tokio::task::JoinHandle<Vec<(String, Value)>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let task = tokio::spawn(async move {
        let mut requests = vec![];
        for _ in 0..count {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = vec![];
            let (end, length) = loop {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|length| length.parse::<usize>().ok())
                        })
                        .unwrap();
                    break (end + 4, length);
                }
            };
            while bytes.len() < end + length {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
            let request: Value = serde_json::from_slice(&bytes[end..end + length]).unwrap();
            let response = respond(&request).to_string();
            requests.push((headers, request));
            stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).as_bytes()).await.unwrap();
        }
        requests
    });
    (url, task)
}

pub fn connection(signed_in: bool) -> Value {
    json!({"data":{"server":{"version":"0.38.0","signInStyle":"PROFILES","setupRequired":false,"email":true},"appearance":{"mode":"SINGLE","style":"LAYERED","light":{"palette":{"tokens":[]}},"dark":{"palette":{"tokens":[]}}},"signInProfiles":[{"key":"opaque-profile","avatar":null}],"viewer": if signed_in { json!({"id":1,"username":"David","avatar":null}) } else { Value::Null }}})
}
