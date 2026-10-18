// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Context;

use crate::auth;
use crate::db::now;
use crate::events::Event;
use crate::media::clip::{self, Recipe};
use crate::media::{burn, screenshot};
use crate::notifications::{self, New};
use crate::state::AppState;

const DEFAULT_FONT: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");

pub const BITMAP_SUBTITLES: &[&str] = &["hdmv_pgs_subtitle", "dvd_subtitle", "dvb_subtitle", "xsub"];

pub struct Clips {
    queue: Mutex<Queue>,
    shots: tokio::sync::Semaphore,
}

impl Default for Clips {
    fn default() -> Self {
        Clips { queue: Default::default(), shots: tokio::sync::Semaphore::new(2) }
    }
}

#[derive(Default)]
struct Queue {
    waiting: VecDeque<(i64, i64)>,
    running: HashMap<i64, Running>,
}

struct Running {
    owner: i64,
    cancel: Arc<AtomicBool>,
    progress: f32,
}

impl Clips {
    pub async fn start(self: &Arc<Self>, state: &Arc<AppState>) {
        let _ = sqlx::query("UPDATE clips SET state = 'queued' WHERE state = 'rendering'").execute(&state.db).await;
        self.resume(state).await;
    }

    pub async fn resume(self: &Arc<Self>, state: &Arc<AppState>) {
        let rows: Vec<(i64, i64, bool)> =
            sqlx::query_as("SELECT id, owner_id, screenshot FROM clips WHERE state = 'queued' ORDER BY id")
                .fetch_all(&state.db)
                .await
                .unwrap_or_default();

        for (id, owner, screenshot) in rows {
            self.render_later(state, id, owner, screenshot);
        }

        self.kick(state);
    }

    pub fn render_later(self: &Arc<Self>, state: &Arc<AppState>, id: i64, owner: i64, screenshot: bool) {
        if screenshot {
            let (this, state) = (self.clone(), state.clone());

            tokio::spawn(async move {
                let _ = this.shoot(&state, id).await;
            });
        } else {
            self.enqueue(state, id, owner);
        }
    }

    pub async fn shoot(self: &Arc<Self>, state: &Arc<AppState>, id: i64) -> anyhow::Result<()> {
        let result = {
            let _permit = self.shots.acquire().await?;
            self.render(state, id, &Arc::new(AtomicBool::new(false))).await
        };

        if let Err(e) = &result {
            tracing::warn!("taking screenshot #{id} failed: {e:#}");

            let _ = sqlx::query("UPDATE clips SET state = 'failed', error = ? WHERE id = ?")
                .bind(format!("{e:#}"))
                .bind(id)
                .execute(&state.db)
                .await;
        }

        changed(state, id).await;
        result
    }

    pub fn enqueue(self: &Arc<Self>, state: &Arc<AppState>, clip: i64, owner: i64) {
        {
            let mut q = self.queue.lock().unwrap();

            if q.running.contains_key(&clip) || q.waiting.iter().any(|(c, _)| *c == clip) {
                return;
            }

            q.waiting.push_back((clip, owner));
        }

        self.kick(state);
    }

    fn kick(self: &Arc<Self>, state: &Arc<AppState>) {
        let config = state.config.current();

        if !config.clips.enabled {
            return;
        }

        let mut q = self.queue.lock().unwrap();

        while q.running.len() < config.clips.concurrency as usize {
            let busy: Vec<i64> = q.running.values().map(|r| r.owner).collect();

            let Some(pos) = q.waiting.iter().position(|(_, owner)| !busy.contains(owner)) else {
                break;
            };

            let (clip, owner) = q.waiting.remove(pos).unwrap();
            let cancel = Arc::new(AtomicBool::new(false));
            q.running.insert(clip, Running { owner, cancel: cancel.clone(), progress: 0.0 });
            let (this, state) = (self.clone(), state.clone());

            tokio::spawn(async move {
                if let Err(e) = this.render(&state, clip, &cancel).await {
                    let stopped = cancel.load(Ordering::Relaxed);

                    let result = if stopped && !state.config.current().clips.enabled {
                        sqlx::query("UPDATE clips SET state = 'queued' WHERE id = ?")
                            .bind(clip)
                            .execute(&state.db)
                            .await
                    } else {
                        if !stopped {
                            tracing::warn!("rendering clip #{clip} failed: {e:#}");
                        }

                        let message = if stopped { "Stopped".to_string() } else { format!("{e:#}") };

                        sqlx::query("UPDATE clips SET state = 'failed', error = ? WHERE id = ?")
                            .bind(message)
                            .bind(clip)
                            .execute(&state.db)
                            .await
                    };

                    if let Err(e) = result {
                        tracing::warn!("{e}");
                    }
                }

                this.queue.lock().unwrap().running.remove(&clip);
                changed(&state, clip).await;
                this.kick(&state);
            });
        }
    }

    pub fn cancel(&self, clip: i64) -> bool {
        let mut q = self.queue.lock().unwrap();
        let before = q.waiting.len();
        q.waiting.retain(|(c, _)| *c != clip);

        if let Some(r) = q.running.get(&clip) {
            r.cancel.store(true, Ordering::Relaxed);
            return true;
        }

        q.waiting.len() != before
    }

    pub fn pause_all(&self) {
        let mut q = self.queue.lock().unwrap();
        q.waiting.clear();

        for r in q.running.values() {
            r.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn progress(&self, clip: i64) -> Option<f32> {
        self.queue.lock().unwrap().running.get(&clip).map(|r| r.progress)
    }

    async fn render(self: &Arc<Self>, state: &Arc<AppState>, id: i64, cancel: &Arc<AtomicBool>) -> anyhow::Result<()> {
        let _busy = state.media.busy.hold();

        let row: Option<(
            i64,
            Option<i64>,
            String,
            i64,
            i64,
            String,
            f64,
            f64,
            Option<i64>,
            Option<String>,
            i64,
            bool,
            Option<i64>,
            bool,
        )> = sqlx::query_as(
            "SELECT owner_id, media_id, source_path, source_size, source_mtime, title, range_start, range_end,
                        audio, subtitles, height, half_rate, rendered_at, screenshot
                 FROM clips WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?;

        let Some((
            owner,
            media_id,
            source,
            size,
            mtime,
            title,
            start,
            end,
            audio,
            subtitles,
            height,
            half_rate,
            rendered_before,
            is_screenshot,
        )) = row
        else {
            return Ok(());
        };

        sqlx::query("UPDATE clips SET state = 'rendering', error = NULL WHERE id = ?")
            .bind(id)
            .execute(&state.db)
            .await?;

        changed(state, id).await;

        let what = if is_screenshot { "screenshot" } else { "clip" };

        let video = source_file(state, media_id, &source)
            .await?
            .with_context(|| format!("the video this {what} came from is gone"))?;

        if fingerprint(&video) != Some((size, mtime)) {
            anyhow::bail!(if is_screenshot {
                "the video this screenshot came from has changed"
            } else {
                "the video this clip came from has changed; open the clip in the editor to check its range"
            });
        }

        let subtitles = match subtitles {
            Some(track) => Some(subtitle_source(state, &video, &track).await?),
            None => None,
        };

        let (fonts_dir, default_font) = fonts(state)?;
        let out = rendered_file(state, owner, id, is_screenshot)?;
        std::fs::create_dir_all(out.parent().unwrap())?;
        let started = Instant::now();

        let (width, height, fps) = if is_screenshot {
            let o = out.clone();

            let shot = tokio::task::spawn_blocking(move || {
                let fonts = burn::Fonts { dir: fonts_dir.as_deref(), default: &default_font };
                let shot = screenshot::take(&video, start, subtitles, &fonts)?;
                std::fs::write(&o, &shot.png)?;
                std::fs::write(o.with_extension("jpg"), &shot.poster)?;
                anyhow::Ok(shot)
            })
            .await??;

            tracing::info!(
                "took screenshot #{id} ({}x{}, {} KB) in {:.1?}",
                shot.width,
                shot.height,
                shot.png.len() / 1024,
                started.elapsed()
            );

            (shot.width, shot.height, None)
        } else {
            let recipe = Recipe {
                start,
                end,
                audio: audio.map(|a| a as usize),
                subtitles,
                height: height as i32,
                half_rate,
                title: title.clone(),
            };

            self.render_clip(state, id, cancel, video, recipe, (fonts_dir, default_font), &out).await?
        };

        let bytes = std::fs::metadata(&out)?.len() as i64;
        let t = now();

        let updated = sqlx::query(
            "UPDATE clips SET state = 'ready', error = NULL, bytes = ?, width_px = ?, height_px = ?, fps = ?,
                rendered_at = ?, viewed_at = ? WHERE id = ?",
        )
        .bind(bytes)
        .bind(width)
        .bind(height)
        .bind(fps)
        .bind(t)
        .bind(t)
        .bind(id)
        .execute(&state.db)
        .await?
        .rows_affected();

        if updated == 0 {
            remove_files(&out);
            return Ok(());
        }

        make_room(state, owner).await?;

        if rendered_before.is_none() && !is_screenshot {
            notifications::send(
                state,
                &[owner],
                New {
                    kind: "clipReady",
                    priority: true,
                    title: "Your clip is ready".into(),
                    body: Some(if title.is_empty() { "Download it, or send it to someone".into() } else { title }),
                    image: Some(format!("/api/clips/{id}/poster?v={t}")),
                    link: Some(format!("/clips?clip={id}")),
                    replace: true,
                    ..Default::default()
                },
            )
            .await;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn render_clip(
        self: &Arc<Self>,
        state: &Arc<AppState>,
        id: i64,
        cancel: &Arc<AtomicBool>,
        video: PathBuf,
        recipe: Recipe,
        (fonts_dir, default_font): (Option<PathBuf>, PathBuf),
        out: &Path,
    ) -> anyhow::Result<(i32, i32, Option<f64>)> {
        let hw = state.media.hw.device();
        let (this, st, cancel2, out2) = (self.clone(), state.clone(), cancel.clone(), out.to_path_buf());
        let started = Instant::now();

        let rendered = tokio::task::spawn_blocking(move || {
            let fonts = burn::Fonts { dir: fonts_dir.as_deref(), default: &default_font };
            let mut last = Instant::now();

            let mut progress = |p: f32| {
                if let Some(r) = this.queue.lock().unwrap().running.get_mut(&id) {
                    r.progress = p;
                }
                if last.elapsed().as_millis() >= 250 {
                    last = Instant::now();
                    let st = st.clone();
                    tokio::runtime::Handle::current().spawn(async move { changed(&st, id).await });
                }
            };

            clip::render(&video, &recipe, &out2, hw.as_ref(), &fonts, &mut progress, &cancel2)
        })
        .await??;

        if cancel.load(Ordering::Relaxed) {
            let _ = std::fs::remove_file(out);
            anyhow::bail!("stopped");
        }

        tracing::info!(
            "rendered clip #{id} ({}x{}, {:.0} fps, {} KB) in {:.1?}",
            rendered.width,
            rendered.height,
            rendered.fps,
            std::fs::metadata(out)?.len() / 1024,
            started.elapsed()
        );

        let (o, poster) = (out.to_path_buf(), out.with_extension("jpg"));

        match tokio::task::spawn_blocking(move || crate::media::thumb::capture(&o, 640)).await? {
            Ok(jpeg) => std::fs::write(&poster, jpeg)?,
            Err(e) => tracing::warn!("no poster for clip #{id}: {e:#}"),
        }

        Ok((rendered.width, rendered.height, Some(rendered.fps)))
    }
}

pub async fn changed(state: &AppState, id: i64) {
    let row: Option<(i64, String)> = sqlx::query_as("SELECT owner_id, state FROM clips WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

    let Some((owner, clip_state)) = row else {
        return;
    };

    let mut users: Vec<i64> = sqlx::query_scalar("SELECT user_id FROM clip_shares WHERE clip_id = ?")
        .bind(id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    users.push(owner);
    state.events.send(Event::ClipChanged { clip_id: id, users, state: clip_state, progress: state.clips.progress(id) });
}

pub fn file(state: &AppState, owner: i64, id: i64) -> anyhow::Result<PathBuf> {
    let config = state.config.current();
    let dir = config.clips.dir(state.config.config_dir(), &state.paths.data_dir)?;
    Ok(dir.join(owner.to_string()).join(format!("{id}.mp4")))
}

pub fn rendered_file(state: &AppState, owner: i64, id: i64, screenshot: bool) -> anyhow::Result<PathBuf> {
    let f = file(state, owner, id)?;
    Ok(if screenshot { f.with_extension("png") } else { f })
}

pub fn remove_files(file: &Path) {
    for ext in ["mp4", "png", "jpg"] {
        let _ = std::fs::remove_file(file.with_extension(ext));
    }
}

async fn evict(state: &AppState, id: i64, owner: i64) -> sqlx::Result<()> {
    if let Ok(f) = file(state, owner, id) {
        remove_files(&f);
    }

    sqlx::query("UPDATE clips SET state = 'evicted', bytes = NULL WHERE id = ? AND state = 'ready'")
        .bind(id)
        .execute(&state.db)
        .await?;

    changed(state, id).await;
    Ok(())
}

pub async fn delete(state: &AppState, id: i64, owner: i64) -> sqlx::Result<()> {
    state.clips.cancel(id);

    if let Ok(f) = file(state, owner, id) {
        remove_files(&f);
    }

    let users: Vec<i64> =
        sqlx::query_scalar("SELECT user_id FROM clip_shares WHERE clip_id = ?").bind(id).fetch_all(&state.db).await?;

    sqlx::query("DELETE FROM clips WHERE id = ?").bind(id).execute(&state.db).await?;
    notifications::withdraw(state, &format!("/clips?clip={id}")).await;
    let mut users = users;
    users.push(owner);
    state.events.send(Event::ClipChanged { clip_id: id, users, state: "deleted".into(), progress: None });
    Ok(())
}

pub async fn make_room(state: &AppState, owner: i64) -> anyhow::Result<()> {
    let config = state.config.current();

    if let Some(user) = auth::load_user(state, owner).await.map_err(|e| anyhow::anyhow!(e.message))? {
        let p = &user.permissions;
        let rows = ready(state, Some(owner)).await?;
        let pinned_links = config.clips.public_links && p.clip_links;
        let (mut count, mut bytes) = (0u64, 0u64);

        for r in rows.iter().filter(|r| r.public && pinned_links) {
            count += !r.screenshot as u64;
            bytes += r.bytes as u64;
        }

        let (limit, space) = (p.clip_limit as u64, p.clip_storage as u64 * 1024 * 1024);

        for r in rows.iter().filter(|r| !(r.public && pinned_links)) {
            let fits = (limit == 0 || r.screenshot || count < limit) && (space == 0 || bytes + r.bytes as u64 <= space);

            if fits {
                count += !r.screenshot as u64;
                bytes += r.bytes as u64;
            } else {
                tracing::info!("dropping the render of clip #{} to make room for {}", r.id, user.username);
                evict(state, r.id, owner).await?;
            }
        }
    }

    if config.clips.max_storage > 0 {
        let ceiling = config.clips.max_storage * 1024 * 1024;
        let rows = ready(state, None).await?;
        let mut total: u64 = rows.iter().map(|r| r.bytes as u64).sum();

        for r in rows.iter().rev() {
            if total <= ceiling {
                break;
            }

            if r.public && pinned(state, r.owner).await {
                continue;
            }

            tracing::info!("dropping the render of clip #{} to stay within [clips] max-storage", r.id);
            evict(state, r.id, r.owner).await?;
            total -= r.bytes as u64;
        }
    }

    Ok(())
}

pub async fn pinned(state: &AppState, owner: i64) -> bool {
    state.config.current().clips.public_links
        && auth::load_user(state, owner).await.ok().flatten().is_some_and(|u| u.permissions.clip_links)
}

struct Ready {
    id: i64,
    owner: i64,
    bytes: i64,
    public: bool,
    screenshot: bool,
}

async fn ready(state: &AppState, owner: Option<i64>) -> sqlx::Result<Vec<Ready>> {
    let rows: Vec<(i64, i64, i64, bool, bool)> = sqlx::query_as(
        "SELECT id, owner_id, COALESCE(bytes, 0), public, screenshot FROM clips
         WHERE state = 'ready' AND quote IS NULL AND (?1 IS NULL OR owner_id = ?1)
         ORDER BY COALESCE(viewed_at, rendered_at) DESC, id DESC",
    )
    .bind(owner)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, owner, bytes, public, screenshot)| Ready { id, owner, bytes, public, screenshot })
        .collect())
}

pub async fn source_file(state: &AppState, media_id: Option<i64>, path: &str) -> sqlx::Result<Option<PathBuf>> {
    let found: Option<String> = match media_id {
        Some(id) => {
            sqlx::query_scalar("SELECT path FROM media WHERE id = ?").bind(id).fetch_optional(&state.db).await?
        },
        None => None,
    };

    let found = match found {
        Some(p) => Some(p),
        None => {
            sqlx::query_scalar("SELECT path FROM media WHERE path = ?").bind(path).fetch_optional(&state.db).await?
        },
    };

    Ok(found.map(PathBuf::from).filter(|p| p.is_file()))
}

pub fn fingerprint(path: &Path) -> Option<(i64, i64)> {
    let m = std::fs::metadata(path).ok()?;
    let mtime = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    Some((m.len() as i64, mtime))
}

pub async fn subtitle_source(state: &AppState, video: &Path, track: &str) -> anyhow::Result<burn::Source> {
    let info = state.media.probe(video).await?;
    let t = info.subtitles.iter().find(|s| s.id == track).context("that subtitle track is gone")?;

    if t.supported {
        let ass = state.media.subtitles(video, track).await?;
        let (v, indexes) = (video.to_path_buf(), info.fonts.iter().map(|f| f.index).collect::<Vec<_>>());

        let fonts = tokio::task::spawn_blocking(move || {
            indexes.into_iter().filter_map(|i| crate::media::probe::attachment(&v, i).ok()).collect::<Vec<_>>()
        })
        .await?;

        return Ok(burn::Source::Text { ass, fonts });
    }

    anyhow::ensure!(BITMAP_SUBTITLES.contains(&t.codec.as_str()), "{} subtitles can't be burned in", t.codec);
    let stream = track.strip_prefix('s').and_then(|s| s.parse().ok()).context("bad subtitle track")?;
    Ok(burn::Source::Bitmap { stream })
}

pub fn fonts(state: &AppState) -> anyhow::Result<(Option<PathBuf>, PathBuf)> {
    let config = state.config.current();
    let dir = config.clips.fonts_dir_path(state.config.config_dir());

    let default = match config.clips.default_font_path(state.config.config_dir()) {
        Some(f) => f,
        None => bundled_font(state)?,
    };

    Ok((dir, default))
}

fn bundled_font(state: &AppState) -> anyhow::Result<PathBuf> {
    let path = state.paths.cache_dir().join("fonts").join("NotoSans-Regular.ttf");

    if std::fs::metadata(&path).map(|m| m.len() as usize).ok() != Some(DEFAULT_FONT.len()) {
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, DEFAULT_FONT)?;
    }

    Ok(path)
}
