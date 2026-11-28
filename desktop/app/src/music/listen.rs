// SPDX-License-Identifier: AGPL-3.0-or-later
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::api::Api;
use crate::player::sync::{Clock, Member, Settings, position_at};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub queue: Vec<i64>,
    pub current: usize,
    pub clock: Clock,
    pub members: Vec<Member>,
    pub host: Host,
    pub public: bool,
    pub settings: Settings,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Host {
    pub id: u64,
    pub name: String,
}
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub room: Option<State>,
    pub you: Option<u64>,
    pub connected: bool,
    pub ended: bool,
    pub error: Option<String>,
    pub offset: f64,
}
impl Snapshot {
    pub fn target(&self) -> Option<(usize, f64, bool)> {
        self.room.as_ref().map(|r| (r.current, position_at(&r.clock, now() + self.offset), r.clock.running))
    }

    pub fn control(&self) -> bool {
        self.room.as_ref().is_some_and(|r| {
            self.you == Some(r.host.id) || r.settings.control == crate::player::sync::Control::Everyone
        })
    }
}
struct Inner {
    snapshot: Snapshot,
    outbox: Option<mpsc::UnboundedSender<Value>>,
    samples: Vec<(f64, f64)>,
}
#[derive(Clone)]
pub struct Client {
    inner: Arc<Mutex<Inner>>,
    cancel: Arc<tokio::sync::Notify>,
}
impl Client {
    pub fn connect(api: Api, code: String, name: Option<String>, notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        let client = Self {
            inner: Arc::new(Mutex::new(Inner { snapshot: Snapshot::default(), outbox: None, samples: vec![] })),
            cancel: Arc::new(tokio::sync::Notify::new()),
        };
        let task = client.clone();
        tokio::spawn(async move {
            let run = task.run(api, code, name, notify);
            tokio::pin!(run);
            let _ = futures::future::select(run, Box::pin(task.cancel.notified())).await;
        });
        client
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.lock().unwrap().snapshot.clone()
    }

    pub fn send(&self, message: Value) {
        if let Some(tx) = &self.inner.lock().unwrap().outbox {
            let _ = tx.send(message);
        }
    }

    pub fn close(&self) {
        self.cancel.notify_one();
    }

    async fn run(&self, api: Api, code: String, name: Option<String>, notify: Arc<dyn Fn() + Send + Sync>) {
        let mut retry = 0u32;
        loop {
            let result = self.session(&api, &code, name.as_deref(), &notify).await;
            {
                let mut inner = self.inner.lock().unwrap();
                inner.outbox = None;
                inner.snapshot.connected = false;
                if let Err(error) = result {
                    inner.snapshot.error = Some(error);
                }
                if inner.snapshot.ended {
                    notify();
                    return;
                }
            }
            notify();
            tokio::time::sleep(Duration::from_millis((500u64 << retry.min(4)).min(10000))).await;
            retry = retry.saturating_add(1);
        }
    }

    async fn session(
        &self,
        api: &Api,
        code: &str,
        name: Option<&str>,
        notify: &Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(), String> {
        let mut url = api.origin.join(&format!("api/listen/{code}/ws")).map_err(|_| "Invalid room link")?;
        url.set_scheme(if api.origin.scheme() == "https" { "wss" } else { "ws" })
            .map_err(|_| "Invalid room address")?;
        if let Some(name) = name {
            url.query_pairs_mut().append_pair("name", name);
        }
        let mut request = url.as_str().into_client_request().map_err(|_| "Invalid room request")?;
        if let Some(token) = &api.token {
            request
                .headers_mut()
                .insert("Authorization", format!("Bearer {}", token.0).parse().map_err(|_| "Invalid token")?);
        }
        let (socket, _) = tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(request))
            .await
            .map_err(|_| "Room connection timed out")?
            .map_err(|_| "Couldn't connect to the room")?;
        let (mut write, mut read) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
        {
            let mut inner = self.inner.lock().unwrap();
            inner.outbox = Some(tx.clone());
            inner.samples.clear();
            inner.snapshot.connected = true;
            inner.snapshot.error = None;
        }
        notify();
        let writer = async {
            let mut ping = 0;
            loop {
                let message = match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
                    Ok(Some(value)) => value,
                    Ok(None) => return Ok::<(), String>(()),
                    Err(_) => {
                        ping += 1;
                        notify();
                        json!({"type":"ping","id":ping,"c":now()})
                    },
                };
                write
                    .send(Frame::Text(message.to_string().into()))
                    .await
                    .map_err(|_| "Room connection closed".to_string())?;
            }
        };
        let reader = async {
            while let Some(frame) = read.next().await {
                let frame = frame.map_err(|_| "Room connection closed")?;
                if let Frame::Text(text) = frame {
                    let message: Value = serde_json::from_str(&text).map_err(|_| "Invalid room response")?;
                    let mut inner = self.inner.lock().unwrap();
                    match message["type"].as_str() {
                        Some("welcome") => {
                            inner.snapshot.you = message["you"].as_u64();
                            inner.snapshot.offset = message["serverTime"].as_f64().unwrap_or(now()) - now();
                        },
                        Some("pong") => {
                            if let (Some(c), Some(server)) = (message["c"].as_f64(), message["s"].as_f64()) {
                                let local = now();
                                inner.samples.push((local - c, server - (local + c) / 2.0));
                                if inner.samples.len() > 8 {
                                    inner.samples.remove(0);
                                }
                                if let Some(best) = inner.samples.iter().min_by(|a, b| a.0.total_cmp(&b.0)) {
                                    inner.snapshot.offset = best.1;
                                }
                            }
                        },
                        Some("state") => {
                            inner.snapshot.room =
                                Some(serde_json::from_value(message).map_err(|_| "Invalid room state")?)
                        },
                        Some("ended") => {
                            inner.snapshot.ended = true;
                            drop(inner);
                            notify();
                            return Ok(());
                        },
                        Some("error") => inner.snapshot.error = message["message"].as_str().map(str::to_owned),
                        _ => {},
                    }
                    drop(inner);
                    notify();
                }
            }
            Ok::<(), String>(())
        };
        match futures::future::select(Box::pin(writer), Box::pin(reader)).await {
            futures::future::Either::Left((r, _)) | futures::future::Either::Right((r, _)) => r,
        }
    }
}
fn now() -> f64 {
    static START: OnceLock<(Instant, f64)> = OnceLock::new();
    let (start, epoch) = START.get_or_init(|| {
        (Instant::now(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64() * 1000.0)
    });
    epoch + start.elapsed().as_secs_f64() * 1000.0
}

impl Drop for Client {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 2 {
            self.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn listening_socket_authenticates_and_follows_web_clock_messages() {
        crate::test_support::run(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = tokio_tungstenite::accept_hdr_async(
                    stream,
                    |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                        assert_eq!(request.uri().path(), "/api/listen/ROOM/ws");
                        assert_eq!(request.headers()["Authorization"], "Bearer room-token");
                        Ok(response)
                    },
                )
                .await
                .unwrap();
                socket
                    .send(Frame::Text(json!({"type":"welcome","you":2,"serverTime":now()}).to_string().into()))
                    .await
                    .unwrap();
                socket.send(Frame::Text(json!({"type":"state","queue":[1,2],"current":1,"host":{"id":2,"name":"Host"},"public":true,"settings":{"control":"everyone","waitForAll":false},"clock":{"paused":false,"running":true,"position":12.0,"at":now(),"rate":1.0},"members":[]}).to_string().into())).await.unwrap();
                while let Some(Ok(Frame::Text(text))) = socket.next().await {
                    let value: Value = serde_json::from_str(&text).unwrap();
                    if value["type"] == "seek" {
                        assert_eq!(value["position"], 19.0);
                        socket.send(Frame::Text(json!({"type":"ended"}).to_string().into())).await.unwrap();
                        break;
                    }
                }
            });
            let api = Api::new(
                url::Url::parse(&format!("http://{address}/")).unwrap(),
                Some(crate::api::Token("room-token".into())),
            )
            .unwrap();
            let client = Client::connect(api, "ROOM".into(), None, Arc::new(|| {}));
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if client.snapshot().room.is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            let snapshot = client.snapshot();
            assert!(snapshot.connected);
            assert!(snapshot.control());
            let (index, time, playing) = snapshot.target().unwrap();
            assert_eq!(index, 1);
            assert!((12.0..13.0).contains(&time));
            assert!(playing);
            client.send(json!({"type":"seek","position":19.0}));
            server.await.unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                while !client.snapshot().ended {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            client.close();
        });
    }
}
