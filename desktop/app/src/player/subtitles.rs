// SPDX-License-Identifier: AGPL-3.0-or-later

//! Server subtitle assets and the renderer that lives beside mpv on the video thread.

use std::sync::Arc;

use tinystream_subtitle_renderer::{Frame, Subtitles};

use crate::api::Api;

pub const DEFAULT_FONT: &[u8] = include_bytes!("../../../../assets/fonts/NotoSans-Regular.ttf");

#[derive(Debug)]
pub struct Document {
    pub script: Vec<u8>,
    pub fonts: Arc<Vec<Vec<u8>>>,
}

#[derive(Default)]
pub struct Loader {
    fonts: Arc<tokio::sync::OnceCell<Arc<Vec<Vec<u8>>>>>,
    pending: Option<futures::future::AbortHandle>,
    revision: u64,
}

impl Loader {
    pub fn cancel(&mut self) -> u64 {
        if let Some(pending) = self.pending.take() {
            pending.abort();
        }
        self.revision += 1;
        self.revision
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn load(
        &mut self,
        api: Api,
        base: String,
        track: String,
        fonts: Vec<i64>,
    ) -> impl Future<Output = super::Message> + Send + 'static + use<> {
        let revision = self.cancel();
        let (abort, registration) = futures::future::AbortHandle::new_pair();
        self.pending = Some(abort);
        let cached = self.fonts.clone();
        async move {
            let result = futures::future::Abortable::new(
                async {
                    let path = format!("{base}/subtitles/{track}");
                    let script = bytes(&api, &path);
                    let fonts = cached.get_or_init(|| async {
                        let fonts = futures::future::join_all(fonts.into_iter().map(|index| {
                            let api = &api;
                            let base = &base;
                            async move { bytes(api, &format!("{base}/fonts/{index}")).await }
                        }))
                        .await;
                        Arc::new(
                            fonts
                                .into_iter()
                                .filter_map(|font| match font {
                                    Ok(font) => Some(font),
                                    Err(error) => {
                                        tracing::warn!("subtitle font: {error}");
                                        None
                                    },
                                })
                                .collect(),
                        )
                    });
                    let (script, fonts) = futures::future::join(script, fonts).await;
                    Ok(Arc::new(Document { script: script?, fonts: fonts.clone() }))
                },
                registration,
            )
            .await;
            super::Message::SubtitleLoaded(revision, result.unwrap_or_else(|_| Err("cancelled".into())))
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel();
    }
}

async fn bytes(api: &Api, path: &str) -> Result<Vec<u8>, String> {
    let response = api
        .media(path)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    Ok(response.bytes().await.map_err(|error| error.to_string())?.to_vec())
}

/// Used only on the video render thread; unchanged images keep their identity through the ring.
#[derive(Default)]
pub struct Overlay {
    renderer: Option<Subtitles>,
    document: Option<Arc<Document>>,
    fonts: Option<Arc<Vec<Vec<u8>>>>,
    layout: Option<[i32; 8]>,
    image: Option<Arc<Frame>>,
}

impl Overlay {
    pub fn clear(&mut self) {
        self.document = None;
        self.image = None;
        if let Some(renderer) = &mut self.renderer {
            renderer.free_track();
        }
    }

    pub fn render(&mut self, document: Option<Arc<Document>>, layout: [i32; 8], time: f64) -> Option<Arc<Frame>> {
        let changed = match (&self.document, &document) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (None, None) => false,
            _ => true,
        };
        if changed {
            self.document = document;
            self.image = None;
            if let Some(document) = &self.document {
                if self.fonts.as_ref().is_none_or(|fonts| !Arc::ptr_eq(fonts, &document.fonts)) {
                    let mut renderer = Subtitles::new(DEFAULT_FONT);
                    for font in document.fonts.iter() {
                        renderer.add_font(font);
                    }
                    self.renderer = Some(renderer);
                    self.fonts = Some(document.fonts.clone());
                }
                let renderer = self.renderer.as_mut()?;
                if let Err(error) = renderer.set_track(&document.script) {
                    renderer.free_track();
                    tracing::warn!("subtitle script: {error}");
                }
            } else {
                self.clear();
            }
        }
        let renderer = self.renderer.as_mut()?;
        let resized = self.layout != Some(layout);
        if resized || changed {
            let [width, height, sw, sh, top, bottom, left, right] = layout;
            renderer.resize(width, height, sw, sh, top, bottom, left, right);
            self.layout = Some(layout);
        }
        if let Some(image) = renderer.render((time * 1000.0).round(), changed || resized) {
            self.image = (image.width > 0).then(|| Arc::new(image));
        }
        self.image.clone()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub const SCRIPT: &[u8] = include_bytes!("../../../../subtitles/tests/data/typesetting.ass");

    pub fn document() -> Arc<Document> {
        Arc::new(Document { script: SCRIPT.to_vec(), fonts: Arc::new(Vec::new()) })
    }

    #[test]
    fn frames_follow_media_time_and_reuse_unchanged_pixels() {
        let document = document();
        let mut overlay = Overlay::default();
        let layout = [640, 360, 640, 360, 0, 0, 0, 0];
        let first = overlay.render(Some(document.clone()), layout, 0.3).unwrap();
        let paused = overlay.render(Some(document.clone()), layout, 0.3).unwrap();
        assert!(Arc::ptr_eq(&first, &paused));
        let karaoke = overlay.render(Some(document.clone()), layout, 0.6).unwrap();
        assert_ne!(first.pixels, karaoke.pixels);
        let seek = overlay.render(Some(document.clone()), layout, 0.3).unwrap();
        assert_eq!(first.pixels, seek.pixels);
        assert!(overlay.render(Some(document.clone()), layout, 1.0).is_none());
        let static_frame = overlay.render(Some(document.clone()), layout, 3.0).unwrap();
        assert!(Arc::ptr_eq(&static_frame, &overlay.render(Some(document.clone()), layout, 3.1).unwrap()));

        let layout = [800, 600, 640, 360, 75, 75, 0, 0];
        let resized = overlay.render(Some(document.clone()), layout, 3.1).unwrap();
        assert!(!Arc::ptr_eq(&static_frame, &resized));
        assert!(resized.y >= 75 && resized.y as u32 + resized.height <= 525);
        assert!(overlay.render(None, layout, 3.1).is_none());
        assert!(overlay.render(Some(document), layout, 3.1).is_some());
    }

    #[test]
    fn loads_authenticated_room_assets_and_cancels_obsolete_requests() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        crate::test_support::run(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}/", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                let mut paths = Vec::new();
                for _ in 0..4 {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut request = Vec::new();
                    loop {
                        let mut buf = [0; 1024];
                        let n = socket.read(&mut buf).await.unwrap();
                        assert_ne!(n, 0);
                        request.extend_from_slice(&buf[..n]);
                        if request.windows(4).any(|b| b == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let request = String::from_utf8(request).unwrap();
                    assert!(request.to_lowercase().contains("authorization: bearer subtitle-test"));
                    let path = request.split_whitespace().nth(1).unwrap().to_owned();
                    assert!(path.starts_with("/api/together/room/media/7/"));
                    let (status, body) = if path.ends_with("fonts/3") {
                        ("200 OK", DEFAULT_FONT)
                    } else if path.ends_with("fonts/4") {
                        ("404 Not Found", &b""[..])
                    } else {
                        ("200 OK", SCRIPT)
                    };
                    socket
                        .write_all(
                            format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())
                                .as_bytes(),
                        )
                        .await
                        .unwrap();
                    socket.write_all(body).await.unwrap();
                    paths.push(path);
                }
                paths
            });
            let api = Api::new(origin.parse().unwrap(), Some(crate::api::Token("subtitle-test".into()))).unwrap();
            let mut loader = Loader::default();
            let base = "api/together/room/media/7".to_owned();
            let cancelled = loader.load(api.clone(), base.clone(), "s9".into(), vec![3, 4]);
            let first = loader.load(api.clone(), base.clone(), "s2".into(), vec![3, 4]);
            assert!(matches!(cancelled.await, super::super::Message::SubtitleLoaded(_, Err(_))));
            let super::super::Message::SubtitleLoaded(_, Ok(a)) = first.await else { panic!("first track failed") };
            let super::super::Message::SubtitleLoaded(_, Ok(b)) = loader.load(api, base, "x5".into(), vec![3, 4]).await
            else {
                panic!("second track failed")
            };
            assert_eq!(a.script, SCRIPT);
            assert_eq!(a.fonts.len(), 1);
            assert!(Arc::ptr_eq(&a.fonts, &b.fonts));
            let paths = server.await.unwrap();
            assert_eq!(paths.iter().filter(|path| path.contains("/fonts/")).count(), 2);
            assert!(paths.iter().all(|path| !path.ends_with("s9")));
        });
    }
}
