// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use graphql_client::{GraphQLQuery, Response};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use url::Url;

#[allow(unused_imports, non_camel_case_types, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/graphql.rs"));
}

pub use generated::*;

#[derive(Clone)]
pub struct Token(pub String);

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

#[derive(Debug, Clone)]
pub struct Error {
    pub unauthorized: bool,
    pub message: String,
}

impl Error {
    pub fn message(message: impl Into<String>) -> Self {
        Self { unauthorized: false, message: message.into() }
    }

    pub fn unauthorized() -> Self {
        Self { unauthorized: true, message: "Your session expired. Sign in again.".into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

#[derive(Clone)]
pub struct Api {
    pub origin: Url,
    pub token: Option<Token>,
    http: Client,
    /// For media streams, which last as long as playback: no overall timeout, but one on each read
    /// so a stream that stops sending is noticed (#66).
    streams: Client,
}

impl fmt::Debug for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Api")
    }
}

impl Api {
    pub fn new(origin: Url, token: Option<Token>) -> Result<Self, Error> {
        let origin = addresses(origin.as_str())?.remove(0);
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("tinystream-desktop/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| Error::message("Couldn't start the network client."))?;
        let streams = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("tinystream-desktop/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| Error::message("Couldn't start the network client."))?;
        Ok(Self { origin, token, http, streams })
    }

    /// Authenticated media requests are restricted to this server's origin.
    pub fn media(&self, path: &str) -> Result<reqwest::RequestBuilder, Error> {
        self.get(&self.http, path)
    }

    /// Like [`Api::media`], for a stream read while it plays.
    pub fn stream(&self, path: &str) -> Result<reqwest::RequestBuilder, Error> {
        self.get(&self.streams, path)
    }

    fn get(&self, client: &Client, path: &str) -> Result<reqwest::RequestBuilder, Error> {
        let url = self.origin.join(path).map_err(|_| Error::message("Invalid media address."))?;
        if url.origin() != self.origin.origin() {
            return Err(Error::message("Invalid media address."));
        }
        let request = client.get(url);
        Ok(match &self.token {
            Some(token) => request.bearer_auth(&token.0),
            None => request,
        })
    }

    pub async fn request<Q: GraphQLQuery>(&self, variables: Q::Variables) -> Result<Q::ResponseData, Error> {
        let mut request =
            self.http.post(self.origin.join("api/graphql").expect("validated origin")).json(&Q::build_query(variables));
        if let Some(token) = &self.token {
            request = request.bearer_auth(&token.0);
        }
        let response = request
            .send()
            .await
            .map_err(|_| Error::message("Couldn't reach the server. Check the address and your connection."))?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(Error::unauthorized());
        }
        if !response.status().is_success() {
            return Err(Error::message(format!("The server returned HTTP {}.", response.status().as_u16())));
        }
        let body: Response<Q::ResponseData> =
            response.json().await.map_err(|_| Error::message("The server sent something unexpected."))?;
        decode(body)
    }

    pub async fn connection(&self) -> Result<connection::ResponseData, Error> {
        let data = self.request::<Connection>(connection::Variables).await?;
        if self.token.is_some() && data.viewer.is_none() {
            return Err(Error::unauthorized());
        }
        Ok(data)
    }

    /// Emits a full invalidation on each acknowledgement, including after reconnecting.
    pub async fn events(&self, send: impl Fn(Live) + Send + Sync) {
        let mut delay = 1;
        loop {
            match self.follow(&send).await {
                Err(error) if error.unauthorized => {
                    send(Live::Unauthorized);
                    return;
                },
                _ => send(Live::Offline),
            }
            sleep(Duration::from_secs(delay)).await;
            delay = (delay * 2).min(30);
        }
    }

    async fn follow(&self, send: &impl Fn(Live)) -> Result<(), Error> {
        let mut url = self.origin.join("api/graphql").expect("validated origin");
        url.set_scheme(if self.origin.scheme() == "https" { "wss" } else { "ws" }).expect("websocket scheme");
        let mut request = url.as_str().into_client_request().map_err(|_| Error::message("Invalid live connection."))?;
        request.headers_mut().insert("Sec-WebSocket-Protocol", "graphql-transport-ws".parse().unwrap());
        let (mut socket, _) = timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(request))
            .await
            .map_err(|_| Error::message("Live connection timed out."))?
            .map_err(|error| match error {
                tokio_tungstenite::tungstenite::Error::Http(response) if response.status().as_u16() == 401 => {
                    Error::unauthorized()
                },
                _ => Error::message("Live connection interrupted."),
            })?;
        socket
            .send(Frame::Text(
                json!({"type":"connection_init", "payload":{"token":self.token.as_ref().map(|t| &t.0)}})
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|_| Error::message("Live connection interrupted."))?;
        let mut acknowledged = false;
        let mut waiting_for_pong = false;
        loop {
            let frame = match timeout(
                Duration::from_secs(if acknowledged && !waiting_for_pong { 30 } else { 15 }),
                socket.next(),
            )
            .await
            {
                Ok(frame) => frame,
                Err(_) if acknowledged && !waiting_for_pong => {
                    socket
                        .send(Frame::Text(json!({"type":"ping"}).to_string().into()))
                        .await
                        .map_err(|_| Error::message("Live connection interrupted."))?;
                    waiting_for_pong = true;
                    continue;
                },
                Err(_) => return Err(Error::message("Live connection timed out.")),
            }
            .ok_or_else(|| Error::message("Live connection closed."))?
            .map_err(|_| Error::message("Live connection interrupted."))?;
            match frame {
                Frame::Text(text) => {
                    let value: Value =
                        serde_json::from_str(&text).map_err(|_| Error::message("Invalid live event."))?;
                    match value["type"].as_str() {
                        Some("connection_ack") if !acknowledged => {
                            acknowledged = true;
                            socket.send(Frame::Text(json!({"id":"events", "type":"subscribe", "payload":Events::build_query(events::Variables)}).to_string().into())).await.map_err(|_| Error::message("Live connection interrupted."))?;
                            send(Live::Connected);
                        },
                        Some("next") if acknowledged && value["id"] == "events" => {
                            let response: Response<events::ResponseData> =
                                serde_json::from_value(value["payload"].clone())
                                    .map_err(|_| Error::message("Invalid live event."))?;
                            send(Live::Event(Arc::new(decode(response)?)));
                        },
                        Some("pong") => waiting_for_pong = false,
                        Some("ping") => {
                            socket
                                .send(Frame::Text(
                                    json!({"type":"pong", "payload":value["payload"]}).to_string().into(),
                                ))
                                .await
                                .map_err(|_| Error::message("Live connection interrupted."))?;
                        },
                        Some("error") => {
                            if value["payload"].as_array().is_some_and(|errors| {
                                errors.iter().any(|e| e["extensions"]["code"] == "UNAUTHENTICATED")
                            }) {
                                return Err(Error::unauthorized());
                            }
                            return Err(Error::message("Live subscription failed."));
                        },
                        Some("complete") => return Ok(()),
                        _ => {},
                    }
                },
                Frame::Ping(bytes) => {
                    socket
                        .send(Frame::Pong(bytes))
                        .await
                        .map_err(|_| Error::message("Live connection interrupted."))?;
                },
                Frame::Close(frame) => {
                    if frame.is_some_and(|f| matches!(u16::from(f.code), 4401 | 4403)) {
                        return Err(Error::unauthorized());
                    }
                    return Ok(());
                },
                _ => {},
            }
        }
    }
}

#[derive(Clone)]
pub enum Live {
    Connected,
    Offline,
    Unauthorized,
    Event(Arc<events::ResponseData>),
}

fn decode<T>(response: Response<T>) -> Result<T, Error> {
    if let Some(errors) = response.errors {
        if let Some(error) = errors.iter().find(|error| {
            error.extensions.as_ref().and_then(|e| e.get("code")).and_then(Value::as_str) == Some("UNAUTHENTICATED")
        }) {
            return Err(Error { unauthorized: true, message: error.message.clone() });
        }
        if let Some(error) = errors.into_iter().next() {
            return Err(Error::message(error.message));
        }
    }
    response.data.ok_or_else(|| Error::message("The server sent nothing back."))
}

/// Bare addresses try HTTPS first. Explicit schemes are respected.
pub fn addresses(input: &str) -> Result<Vec<Url>, Error> {
    let input = input.trim();
    if input.is_empty() {
        return Err(Error::message("Enter a server address."));
    }
    let candidates = if input.contains("://") {
        vec![input.to_owned()]
    } else {
        vec![format!("https://{input}"), format!("http://{input}")]
    };
    candidates
        .into_iter()
        .map(|candidate| {
            let mut url =
                Url::parse(&candidate).map_err(|_| Error::message("Enter a host, host:port, or an HTTP(S) URL."))?;
            if !matches!(url.scheme(), "https" | "http")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || !matches!(url.path(), "" | "/")
            {
                return Err(Error::message(
                    "Use the server's HTTP(S) address without a path, password, query, or fragment.",
                ));
            }
            url.set_path("/");
            Ok(url)
        })
        .collect()
}

pub async fn probe(input: &str) -> Result<(Api, connection::ResponseData), Error> {
    let mut last = Error::message("This address isn't a tinystream server.");
    for origin in addresses(input)? {
        let api = Api::new(origin, None)?;
        match api.connection().await {
            Ok(data) if semver::Version::parse(data.server.version.trim_start_matches('v')).is_ok() => {
                return Ok((api, data));
            },
            Ok(_) => last = Error::message("This address isn't a tinystream server."),
            Err(error) => last = error,
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_addresses_and_rejects_credentials_and_paths() {
        let candidates = addresses(" localhost:3000 ").unwrap();
        assert_eq!(
            candidates.iter().map(Url::as_str).collect::<Vec<_>>(),
            ["https://localhost:3000/", "http://localhost:3000/"]
        );
        assert_eq!(addresses("http://[::1]:3000").unwrap().len(), 1);
        for invalid in [
            "",
            "ftp://example.com",
            "https://user:password@example.com",
            "https://example.com/subpath",
            "https://example.com/#secret",
        ] {
            assert!(addresses(invalid).is_err());
        }
    }

    #[test]
    fn graphql_auth_errors_sign_out_even_with_partial_data() {
        let response: Response<Value> = serde_json::from_value(
            json!({"data":{},"errors":[{"message":"sign in","extensions":{"code":"UNAUTHENTICATED"}}]}),
        )
        .unwrap();
        let error = decode(response).unwrap_err();
        assert!(error.unauthorized);
        assert_eq!(error.message, "sign in");
        assert_eq!(format!("{:?}", Token("secret".into())), "[redacted]");
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::test_support::{self, run};

    #[test]
    fn http_uses_bearer_auth_and_detects_a_revoked_session() {
        run(async {
            let (origin, server) = test_support::http(1, |_| test_support::connection(false)).await;
            let api = Api::new(origin, Some(Token("test-session".into()))).unwrap();
            assert!(matches!(api.connection().await, Err(Error { unauthorized: true, .. })));
            let requests = server.await.unwrap();
            assert!(requests[0].0.to_lowercase().contains("authorization: bearer test-session"));
            assert_eq!(requests[0].1["operationName"], "Connection");
            assert!(api.media("https://elsewhere.example/api/media/1/stream").is_err());
        });
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn websocket_authenticates_reconnects_and_delivers_events() {
        run(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
            let server = tokio::spawn(async move {
                for round in 0..2 {
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut socket = tokio_tungstenite::accept_hdr_async(stream, |request: &tokio_tungstenite::tungstenite::handshake::server::Request, mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                        assert_eq!(request.headers()["Sec-WebSocket-Protocol"], "graphql-transport-ws");
                        response.headers_mut().insert("Sec-WebSocket-Protocol", "graphql-transport-ws".parse().unwrap());
                        Ok(response)
                    }).await.unwrap();
                    let init: Value =
                        serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
                    assert_eq!(init["payload"]["token"], "test-session");
                    socket.send(Frame::Text(json!({"type":"connection_ack"}).to_string().into())).await.unwrap();
                    let subscribe: Value =
                        serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
                    assert_eq!(subscribe["payload"]["operationName"], "Events");
                    if round == 0 {
                        socket.send(Frame::Text(json!({"id":"events","type":"next","payload":{"data":{"events":{"__typename":"ConfigChanged"}}}}).to_string().into())).await.unwrap();
                        socket
                            .send(Frame::Text(json!({"type":"ping","payload":{"test":true}}).to_string().into()))
                            .await
                            .unwrap();
                        let pong: Value =
                            serde_json::from_str(socket.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
                        assert_eq!(pong, json!({"type":"pong","payload":{"test":true}}));
                        socket.close(None).await.unwrap();
                    } else {
                        socket.send(Frame::Text(json!({"id":"events","type":"error","payload":[{"message":"sign in","extensions":{"code":"UNAUTHENTICATED"}}]}).to_string().into())).await.unwrap();
                    }
                }
            });
            let events = std::sync::Mutex::new(vec![]);
            let api = Api::new(origin, Some(Token("test-session".into()))).unwrap();
            timeout(Duration::from_secs(8), api.events(|event| events.lock().unwrap().push(event))).await.unwrap();
            server.await.unwrap();
            let events = events.into_inner().unwrap();
            assert_eq!(events.iter().filter(|e| matches!(e, Live::Connected)).count(), 2);
            assert_eq!(events.iter().filter(|e| matches!(e, Live::Event(_))).count(), 1);
            assert!(matches!(events.last(), Some(Live::Unauthorized)));
        });
    }
}
