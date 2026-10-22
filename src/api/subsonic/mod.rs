// SPDX-License-Identifier: AGPL-3.0-or-later

mod browse;
mod encode;
mod files;
mod people;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, RawQuery, Request, State};
use axum::http::{HeaderMap, Method, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use md5::{Digest, Md5};
use serde_json::{Map, Value, json};
use sha2::Sha256;

use crate::auth::{self, User};
use crate::db::now;
use crate::state::AppState;

pub const API_VERSION: &str = "1.16.1";

#[derive(Debug)]
pub struct Failure {
    code: u32,
    message: String,
}

impl Failure {
    pub fn new(code: u32, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn missing(name: &str) -> Self {
        Self::new(10, format!("required parameter is missing: {name}"))
    }

    pub fn not_found(what: &str) -> Self {
        Self::new(70, format!("{what} not found"))
    }

    pub fn forbidden() -> Self {
        Self::new(50, "you aren't allowed to do that")
    }
}

impl From<sqlx::Error> for Failure {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("database error: {e}");
        Self::new(0, "database error, see the server log")
    }
}

impl From<anyhow::Error> for Failure {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("{e:#}");
        Self::new(0, format!("{e:#}"))
    }
}

impl From<crate::error::ApiError> for Failure {
    fn from(e: crate::error::ApiError) -> Self {
        let code = match e.status.as_u16() {
            404 => 70,
            401 | 403 => 50,
            400 => 10,
            _ => 0,
        };

        Self::new(code, e.message)
    }
}

pub type Result<T> = std::result::Result<T, Failure>;

/// What an endpoint answers: fields for the response, or something else entirely, like a file.
pub enum Reply {
    Fields(Map<String, Value>),
    Raw(Response),
}

impl Reply {
    pub fn empty() -> Self {
        Reply::Fields(Map::new())
    }

    pub fn one(key: &str, value: Value) -> Self {
        let mut m = Map::new();
        m.insert(key.into(), value);
        Reply::Fields(m)
    }
}

/// The request's parameters, from the query and, for POST, a form body; names can repeat.
pub struct Params(Vec<(String, String)>);

impl Params {
    fn parse(query: Option<&str>, body: &[u8]) -> Self {
        let mut all: Vec<(String, String)> = Vec::new();

        for src in [query.unwrap_or("").as_bytes(), body] {
            for (k, v) in url::form_urlencoded::parse(src) {
                let k = k.trim_end_matches("[]").to_string();
                all.push((k, v.into_owned()));
            }
        }

        Params(all)
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn all(&self, name: &str) -> Vec<&str> {
        self.0.iter().filter(|(k, _)| k == name).map(|(_, v)| v.as_str()).collect()
    }

    pub fn require(&self, name: &str) -> Result<&str> {
        self.get(name).ok_or_else(|| Failure::missing(name))
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        self.get(name).and_then(|v| v.trim().parse().ok())
    }

    pub fn int_or(&self, name: &str, default: i64) -> i64 {
        self.int(name).unwrap_or(default)
    }

    pub fn flag(&self, name: &str) -> Option<bool> {
        self.get(name).map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
    }
}

/// What a request comes with: who's asking, from which app.
pub struct Call {
    pub user: User,
    pub client: String,
    pub params: Params,
    pub body: Bytes,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new().route("/rest/{endpoint}", get(rest).post(rest)).with_state(state)
}

/// Checking a password with argon2 takes a while and apps send one with
/// every request, so ones that worked are remembered for a bit.
fn remembered(state: &AppState, key: [u8; 32]) -> Option<i64> {
    let map = state.music.logins.lock().unwrap();
    map.get(&key).filter(|(_, at)| at.elapsed() < Duration::from_secs(600)).map(|(id, _)| *id)
}

fn remember(state: &AppState, key: [u8; 32], user_id: i64) {
    let mut map = state.music.logins.lock().unwrap();

    if map.len() > 1024 {
        map.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(600));
    }

    map.insert(key, (user_id, Instant::now()));
}

fn decode_password(p: &str) -> String {
    match p.strip_prefix("enc:") {
        Some(hex_value) => hex::decode(hex_value).ok().and_then(|b| String::from_utf8(b).ok()).unwrap_or_default(),
        None => p.to_string(),
    }
}

async fn used(state: &AppState, id: i64, client: &str) {
    let _ = sqlx::query("UPDATE app_passwords SET last_used = ?, client = ? WHERE id = ? AND (last_used IS NULL OR last_used < ? OR client IS NOT ?)")
        .bind(now())
        .bind(client)
        .bind(id)
        .bind(now() - 300)
        .bind(client)
        .execute(&state.db)
        .await;
}

async fn sign_in(state: &AppState, params: &Params, client: &str) -> Result<User> {
    let api_key = params.get("apiKey");
    let username = params.get("u");

    if let Some(key) = api_key {
        if username.is_some() {
            return Err(Failure::new(43, "use either an API key or a username, not both"));
        }

        let row: Option<(i64, i64)> = sqlx::query_as("SELECT id, user_id FROM app_passwords WHERE secret = ?")
            .bind(key)
            .fetch_optional(&state.db)
            .await?;

        let (id, user_id) = row.ok_or_else(|| Failure::new(44, "that API key isn't valid"))?;
        used(state, id, client).await;
        return auth::load_user(state, user_id).await?.ok_or_else(|| Failure::new(44, "that API key isn't valid"));
    }

    let username = username.ok_or_else(|| Failure::missing("u"))?;
    let wrong = || Failure::new(40, "wrong username or password");

    let account: Option<(i64, String)> = sqlx::query_as("SELECT id, password_hash FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(&state.db)
        .await?;

    let (user_id, hash) = account.ok_or_else(wrong)?;

    let apps: Vec<(i64, String)> = sqlx::query_as("SELECT id, secret FROM app_passwords WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(&state.db)
        .await?;

    let ok = if let (Some(token), Some(salt)) = (params.get("t"), params.get("s")) {
        let token = token.to_ascii_lowercase();
        let found = apps.iter().find(|(_, secret)| hex::encode(Md5::digest(format!("{secret}{salt}"))) == token);

        match found {
            Some((id, _)) => {
                used(state, *id, client).await;
                true
            },
            None => false,
        }
    } else if let Some(p) = params.get("p") {
        let password = decode_password(p);

        if let Some((id, _)) = apps.iter().find(|(_, secret)| *secret == password) {
            used(state, *id, client).await;
            true
        } else {
            let key: [u8; 32] = Sha256::digest(format!("{user_id}\0{password}\0{hash}")).into();

            if remembered(state, key) == Some(user_id) {
                true
            } else if auth::verify_password(password, hash).await {
                remember(state, key, user_id);
                true
            } else {
                false
            }
        }
    } else {
        return Err(Failure::new(42, "sign in with a password, a token, or an API key"));
    };

    if !ok {
        tracing::info!("a music app ({client}) failed to sign in as {username}");
        return Err(wrong());
    }

    auth::load_user(state, user_id).await?.ok_or_else(wrong)
}

fn base() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("status".into(), json!("ok"));
    m.insert("version".into(), json!(API_VERSION));
    m.insert("type".into(), json!("tinystream"));
    m.insert("serverVersion".into(), json!(env!("TINYSTREAM_VERSION")));
    m.insert("openSubsonic".into(), json!(true));
    m
}

pub fn respond(params: &Params, result: Result<Reply>) -> Response {
    let format = params.get("f").unwrap_or("xml");
    let mut body = base();

    match result {
        Ok(Reply::Raw(res)) => return res,
        Ok(Reply::Fields(fields)) => body.extend(fields),
        Err(f) => {
            body.insert("status".into(), json!("failed"));
            body.insert("error".into(), json!({ "code": f.code, "message": f.message }));
        },
    }

    match format {
        "json" => {
            let text = json!({ "subsonic-response": body }).to_string();
            ([(header::CONTENT_TYPE, "application/json")], text).into_response()
        },
        "jsonp" => {
            let callback = params.get("callback").filter(|c| c.chars().all(|c| c.is_alphanumeric() || c == '_'));
            let text = format!("{}({})", callback.unwrap_or("callback"), json!({ "subsonic-response": body }));
            ([(header::CONTENT_TYPE, "application/javascript")], text).into_response()
        },
        _ => ([(header::CONTENT_TYPE, "text/xml; charset=utf-8")], encode::xml("subsonic-response", &body))
            .into_response(),
    }
}

async fn rest(
    State(state): State<Arc<AppState>>,
    Path(endpoint): Path<String>,
    RawQuery(query): RawQuery,
    method: Method,
    headers: HeaderMap,
    req: Request,
) -> Response {
    let name = endpoint.strip_suffix(".view").unwrap_or(&endpoint).to_string();

    let form = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded"));

    let streams = matches!(name.as_str(), "stream" | "download");

    let (req, body) = if method == Method::POST && !streams {
        let bytes = axum::body::to_bytes(req.into_body(), 1 << 20).await.unwrap_or_default();
        (None, bytes)
    } else {
        (Some(req), Bytes::new())
    };

    let params = Params::parse(query.as_deref(), if form { &body } else { &[] });
    let client = params.get("c").unwrap_or("an app").chars().take(60).collect::<String>();

    if matches!(name.as_str(), "getOpenSubsonicExtensions" | "getLicense" | "ping")
        && params.get("u").is_none()
        && params.get("apiKey").is_none()
    {
        let result = Box::pin(dispatch(&state, &name, None, &params, &body, req)).await;
        return respond(&params, result);
    }

    let user = match sign_in(&state, &params, &client).await {
        Ok(u) => u,
        Err(f) => return respond(&params, Err(f)),
    };

    let result = Box::pin(dispatch(&state, &name, Some((user, client)), &params, &body, req)).await;

    if let Err(f) = &result
        && f.code == 0
    {
        tracing::warn!("music API {name}: {}", f.message);
    }

    respond(&params, result)
}

async fn dispatch(
    state: &Arc<AppState>,
    name: &str,
    who: Option<(User, String)>,
    p: &Params,
    body: &Bytes,
    req: Option<Request>,
) -> Result<Reply> {
    match name {
        "ping" => return Ok(Reply::empty()),
        "getLicense" => {
            return Ok(Reply::one(
                "license",
                json!({ "valid": true, "email": "", "licenseExpires": "2099-12-31T00:00:00Z" }),
            ));
        },
        "getOpenSubsonicExtensions" => return Ok(Reply::one("openSubsonicExtensions", extensions())),
        _ => {},
    }

    let Some((user, client)) = who else { return Err(Failure::missing("u")) };
    let call = Call { user, client, params: Params(p.0.clone()), body: body.clone() };

    match name {
        "getMusicFolders" => browse::music_folders(state, &call).await,
        "getIndexes" => browse::indexes(state, &call).await,
        "getMusicDirectory" => browse::music_directory(state, &call).await,
        "getGenres" => browse::genres(state, &call).await,
        "getArtists" => browse::artists(state, &call).await,
        "getArtist" => browse::artist(state, &call).await,
        "getAlbum" => browse::album(state, &call).await,
        "getSong" => browse::song(state, &call).await,
        "getArtistInfo" | "getArtistInfo2" => browse::artist_info(state, &call, name).await,
        "getAlbumInfo" | "getAlbumInfo2" => browse::album_info(state, &call).await,
        "getSimilarSongs" | "getSimilarSongs2" => browse::similar_songs(state, &call, name).await,
        "getTopSongs" => browse::top_songs(state, &call).await,
        "getAlbumList" | "getAlbumList2" => browse::album_list(state, &call, name).await,
        "getRandomSongs" => browse::random_songs(state, &call).await,
        "getSongsByGenre" => browse::songs_by_genre(state, &call).await,
        "getStarred" | "getStarred2" => browse::starred(state, &call, name).await,
        "search2" | "search3" => browse::search(state, &call, name).await,
        "getNowPlaying" => browse::now_playing(state, &call).await,
        "getPlaylists" => people::playlists(state, &call).await,
        "getPlaylist" => people::playlist(state, &call).await,
        "createPlaylist" => people::create_playlist(state, &call).await,
        "updatePlaylist" => people::update_playlist(state, &call).await,
        "deletePlaylist" => people::delete_playlist(state, &call).await,
        "star" | "unstar" => people::star(state, &call, name == "star").await,
        "setRating" => people::set_rating(state, &call).await,
        "scrobble" => people::scrobble(state, &call).await,
        "reportPlayback" => people::report_playback(state, &call).await,
        "getPlayQueue" | "getPlayQueueByIndex" => people::play_queue(state, &call, name).await,
        "savePlayQueue" | "savePlayQueueByIndex" => people::save_play_queue(state, &call, name).await,
        "getUser" => people::user(state, &call).await,
        "getUsers" => people::users(state, &call).await,
        "getScanStatus" => people::scan_status(state, &call).await,
        "startScan" => people::start_scan(state, &call).await,
        "stream" => files::stream(state, &call, req).await,
        "download" => files::download(state, &call, req).await,
        "getCoverArt" => files::cover_art(state, &call).await,
        "getAvatar" => files::avatar(state, &call).await,
        "getLyrics" => files::lyrics(state, &call).await,
        "getLyricsBySongId" => files::lyrics_by_song(state, &call).await,
        "getTranscodeDecision" => files::transcode_decision(state, &call).await,
        "getTranscodeStream" => files::transcode_stream(state, &call).await,
        "getInternetRadioStations" => Ok(Reply::one("internetRadioStations", json!({ "internetRadioStation": [] }))),
        "getPodcasts" => Ok(Reply::one("podcasts", json!({ "channel": [] }))),
        "getNewestPodcasts" => Ok(Reply::one("newestPodcasts", json!({ "episode": [] }))),
        "getBookmarks" => Ok(Reply::one("bookmarks", json!({ "bookmark": [] }))),
        "getShares" => Ok(Reply::one("shares", json!({ "share": [] }))),
        "getVideos" => Ok(Reply::one("videos", json!({ "video": [] }))),
        "getChatMessages" => Ok(Reply::one("chatMessages", json!({ "chatMessage": [] }))),
        "jukeboxControl" => Err(Failure::new(0, "this server doesn't have a jukebox")),
        _ => Err(Failure::new(0, format!("{name} isn't something this server does"))),
    }
}

fn extensions() -> Value {
    json!([
        { "name": "apiKeyAuthentication", "versions": [1] },
        { "name": "formPost", "versions": [1] },
        { "name": "songLyrics", "versions": [1] },
        { "name": "transcodeOffset", "versions": [1] },
        { "name": "transcoding", "versions": [1] },
        { "name": "indexBasedQueue", "versions": [1] },
        { "name": "playbackReport", "versions": [1] },
        { "name": "topSongsByArtistId", "versions": [1] },
    ])
}
