// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use notify::{RecursiveMode, Watcher};
use tokio::sync::mpsc;

use super::parse::{self, FileKind};
use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

#[derive(Clone)]
pub struct Scanner {
    tx: mpsc::UnboundedSender<String>,
}

impl Scanner {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<String>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }

    pub fn request(&self, library: &str) {
        let _ = self.tx.send(library.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Show,
    Movie,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Show => "show",
            Kind::Movie => "movie",
        }
    }
}

#[derive(Debug)]
struct FoundItem {
    path: PathBuf,
    kind: Kind,
    title: String,
    year: Option<i64>,
    media: Vec<FoundMedia>,
}

#[derive(Debug)]
struct FoundMedia {
    path: PathBuf,
    ep: Option<parse::EpisodeNumber>,
    season: Option<u32>,
    size: i64,
    mtime: i64,
}

#[derive(Default)]
struct Walk {
    items: Vec<FoundItem>,
    skipped: Vec<(PathBuf, String)>,
}

impl Walk {
    fn skip(&mut self, path: &Path, reason: impl Into<String>) {
        self.skipped.push((path.to_path_buf(), reason.into()));
    }
}

fn sorted_entries(dir: &Path) -> std::io::Result<Vec<std::fs::DirEntry>> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    Ok(entries)
}

fn is_hidden(entry: &std::fs::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

pub(super) fn file_info(path: &Path) -> (i64, i64) {
    std::fs::metadata(path)
        .map(|m| {
            let mtime = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok());
            (m.len() as i64, mtime.map(|d| d.as_secs() as i64).unwrap_or(0))
        })
        .unwrap_or((0, 0))
}

#[cfg(feature = "torrent")]
fn download_dirs(state: &AppState, config: &crate::config::Config) -> Vec<PathBuf> {
    crate::automation::engine::download_dirs(state, config)
}

#[cfg(not(feature = "torrent"))]
fn download_dirs(_state: &AppState, _config: &crate::config::Config) -> Vec<PathBuf> {
    Vec::new()
}

fn is_download_dir(path: &Path, download_dirs: &[PathBuf]) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    download_dirs.iter().any(|d| d == &path)
}

fn walk(root: &Path, download_dirs: &[PathBuf]) -> std::io::Result<Walk> {
    let mut walk = Walk::default();

    for entry in sorted_entries(root)? {
        let path = entry.path();

        if is_hidden(&entry) || is_download_dir(&path, download_dirs) {
            continue;
        }

        let Ok(ft) = entry.file_type() else { continue };
        let is_dir = ft.is_dir() || (ft.is_symlink() && path.is_dir());

        if !is_dir {
            if matches!(parse::file_kind(&path), FileKind::Video) {
                walk.skip(&path, "video at the library root; put it in a folder: `Title (Year)/file` for a movie, `Show/Season 01/… S01E01` for a show");
            }

            continue;
        }

        walk_title_folder(&path, &mut walk);
    }

    Ok(walk)
}

fn walk_title_folder(dir: &Path, walk: &mut Walk) {
    let Ok(entries) = sorted_entries(dir) else {
        return walk.skip(dir, "can't read this folder (permissions?)");
    };

    let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    let (title, year) = parse::title_and_year(&name);

    let mut seasons = Vec::new();
    let mut other_dirs = Vec::new();
    let mut videos = Vec::new();

    for entry in entries {
        if is_hidden(&entry) {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            let dir_name = entry.file_name().to_string_lossy().to_string();

            match parse::season_number(&dir_name) {
                Some(n) => seasons.push((n, path)),
                None => other_dirs.push(path),
            }
        } else if matches!(parse::file_kind(&path), FileKind::Video) {
            videos.push(path);
        }
    }

    if !seasons.is_empty() {
        for v in &videos {
            walk.skip(v, "video outside a season folder; move it into e.g. `Season 01`");
        }

        for d in &other_dirs {
            walk.skip(d, "folder isn't a season folder (expected a name like `Season 01`, `S01` or `Specials`)");
        }

        let mut media = Vec::new();
        let mut seen: HashMap<(u32, u32), PathBuf> = HashMap::new();

        for (folder_season, season_dir) in seasons {
            walk_season_folder(&season_dir, folder_season, &mut media, &mut seen, walk);
        }

        if media.is_empty() {
            return walk.skip(dir, "show has season folders but no playable episodes or extras");
        }

        walk.items.push(FoundItem { path: dir.to_path_buf(), kind: Kind::Show, title, year, media });
    } else if videos
        .iter()
        .filter(|v| parse::episode_number(&v.file_stem().unwrap_or_default().to_string_lossy()).is_some())
        .count()
        >= 2
    {
        walk.skip(
            dir,
            "looks like a show, but the episodes aren't in a season folder; move them into e.g. `Season 01`",
        );
    } else if !videos.is_empty() {
        let (samples, mut mains): (Vec<_>, Vec<_>) =
            videos.into_iter().partition(|v| parse::is_sample(&v.file_stem().unwrap_or_default().to_string_lossy()));

        for s in &samples {
            walk.skip(s, "sample file");
        }

        if mains.is_empty() {
            return walk.skip(dir, "movie folder only contains samples");
        }

        mains.sort_by_key(|p| std::cmp::Reverse(file_info(p).0));
        let main = mains.remove(0);

        for extra in &mains {
            walk.skip(
                extra,
                format!(
                    "extra video in a movie folder; only the largest file ({}) is used",
                    main.file_name().unwrap_or_default().to_string_lossy()
                ),
            );
        }

        for d in &other_dirs {
            walk.skip(d, "folders inside a movie folder are ignored");
        }

        let (size, mtime) = file_info(&main);

        walk.items.push(FoundItem {
            path: dir.to_path_buf(),
            kind: Kind::Movie,
            title,
            year,
            media: vec![FoundMedia { path: main, ep: None, season: None, size, mtime }],
        });
    } else {
        walk.skip(dir, "no season folders and no videos inside; not a show or a movie");
    }
}

fn walk_season_folder(
    dir: &Path,
    folder_season: u32,
    media: &mut Vec<FoundMedia>,
    seen: &mut HashMap<(u32, u32), PathBuf>,
    walk: &mut Walk,
) {
    let Ok(entries) = sorted_entries(dir) else {
        return walk.skip(dir, "can't read this folder (permissions?)");
    };

    for entry in entries {
        if is_hidden(&entry) {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            if !entry.file_type().is_ok_and(|ft| ft.is_symlink()) {
                walk_extras(&path, folder_season, media, walk);
            }

            continue;
        }

        match parse::file_kind(&path) {
            FileKind::Video => {},
            FileKind::Quiet => continue,
            FileKind::Other => {
                tracing::debug!("ignoring {} (not a video)", path.display());
                continue;
            },
        }

        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();

        if parse::is_sample(&stem) {
            walk.skip(&path, "sample file");
            continue;
        }

        let Some(mut ep) = parse::episode_number(&stem) else {
            if folder_season == 0 {
                let (size, mtime) = file_info(&path);
                media.push(FoundMedia { path, ep: None, season: Some(0), size, mtime });
                continue;
            }

            walk.skip(&path, "no SxxEyy episode number in the file name (e.g. `Show S01E05.mkv`)");
            continue;
        };

        if ep.season != folder_season {
            tracing::debug!(
                "{} says season {} but lives in a season {} folder; the folder wins",
                path.display(),
                ep.season,
                folder_season
            );

            ep.season = folder_season;
        }

        if let Some(first) = seen.get(&(ep.season, ep.episode)) {
            walk.skip(
                &path,
                format!(
                    "duplicate of S{:02}E{:02} ({})",
                    ep.season,
                    ep.episode,
                    first.file_name().unwrap_or_default().to_string_lossy()
                ),
            );

            continue;
        }

        seen.insert((ep.season, ep.episode), path.clone());
        let (size, mtime) = file_info(&path);
        media.push(FoundMedia { path, ep: Some(ep), season: Some(folder_season), size, mtime });
    }
}

fn walk_extras(dir: &Path, season: u32, media: &mut Vec<FoundMedia>, walk: &mut Walk) {
    let Ok(entries) = sorted_entries(dir) else {
        return walk.skip(dir, "can't read this folder (permissions?)");
    };

    for entry in entries {
        if is_hidden(&entry) {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            if !entry.file_type().is_ok_and(|ft| ft.is_symlink()) {
                walk_extras(&path, season, media, walk);
            }
        } else if matches!(parse::file_kind(&path), FileKind::Video) {
            if parse::is_sample(&path.file_stem().unwrap_or_default().to_string_lossy()) {
                walk.skip(&path, "sample file");
                continue;
            }

            let (size, mtime) = file_info(&path);
            media.push(FoundMedia { path, ep: None, season: Some(season), size, mtime });
        }
    }
}

pub fn spawn_worker(state: Arc<AppState>, mut rx: mpsc::UnboundedReceiver<String>) {
    tokio::spawn(async move {
        while let Some(first) = rx.recv().await {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut pending = vec![first];

            while let Ok(more) = rx.try_recv() {
                if !pending.contains(&more) {
                    pending.push(more);
                }
            }

            for library in pending {
                if let Err(e) = scan_library(&state, &library).await {
                    tracing::error!("scanning {library:?} failed: {e:#}");
                }
            }
        }
    });
}

async fn scan_library(state: &Arc<AppState>, name: &str) -> anyhow::Result<()> {
    let config = state.config.current();
    let Some(library) = config.library(name) else { return Ok(()) };

    let root = match library.resolved_path(state.config.config_dir()) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("library {name:?}: {e:#}");
            return Ok(());
        },
    };

    if !root.is_dir() {
        tracing::error!(
            "library {name:?}: {} doesn't exist or isn't a folder; keeping what was already scanned",
            root.display()
        );

        return Ok(());
    }

    state.events.send(Event::ScanStarted { library: name.to_string() });

    if library.is_music() {
        return super::music::scan(state, name, &root, download_dirs(state, &config)).await;
    }

    if library.is_books() {
        return super::books::scan(state, name, &root, download_dirs(state, &config)).await;
    }

    let started = std::time::Instant::now();

    let walk = {
        let root = root.clone();
        let download_dirs = download_dirs(state, &config);
        tokio::task::spawn_blocking(move || walk(&root, &download_dirs)).await??
    };

    let db = &state.db;

    let existing: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM items WHERE library = ?").bind(name).fetch_one(db).await?;

    if walk.items.is_empty() && existing > 0 {
        tracing::warn!(
            "library {name:?}: {} is empty; if it's a drive that isn't mounted, that's why. \
             Keeping the {existing} titles already scanned",
            root.display()
        );

        return Ok(());
    }

    let ts = now();
    let mut tx = db.begin().await?;
    let mut keep_items = HashSet::new();
    let mut new_media = 0usize;
    let mut media_count = 0usize;

    for item in &walk.items {
        let path = item.path.to_string_lossy().to_string();

        let item_id: i64 = sqlx::query_scalar(
            "INSERT INTO items (library, kind, path, folder_title, folder_year, title, sort_title, year, added_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?4, ?6, ?5, ?7, ?7)
             ON CONFLICT(path) DO UPDATE SET
                library = excluded.library,
                kind = excluded.kind,
                folder_title = excluded.folder_title,
                folder_year = excluded.folder_year,
                title = CASE WHEN items.match_state IN ('pending', 'unmatched') THEN excluded.title ELSE items.title END,
                sort_title = CASE WHEN items.match_state IN ('pending', 'unmatched') THEN excluded.sort_title ELSE items.sort_title END
             RETURNING id",
        )
        .bind(name)
        .bind(item.kind.as_str())
        .bind(&path)
        .bind(&item.title)
        .bind(item.year)
        .bind(parse::sort_title(&item.title))
        .bind(ts)
        .fetch_one(&mut *tx)
        .await?;

        keep_items.insert(item_id);

        let mut keep_media = Vec::new();

        for m in &item.media {
            media_count += 1;

            let (season, episode, episode_end) = match m.ep {
                Some(ep) => (m.season.map(i64::from), Some(ep.episode as i64), ep.episode_end.map(|e| e as i64)),
                None => (m.season.map(i64::from), None, None),
            };

            let (id, inserted): (i64, bool) = sqlx::query_as(
                "INSERT INTO media (item_id, path, season, episode, episode_end, size, mtime, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(path) DO UPDATE SET
                    item_id = excluded.item_id,
                    season = excluded.season,
                    episode = excluded.episode,
                    episode_end = excluded.episode_end,
                    duration = CASE WHEN media.size = excluded.size AND media.mtime = excluded.mtime THEN media.duration ELSE NULL END,
                    size = excluded.size,
                    mtime = excluded.mtime
                 RETURNING id, added_at = ?8",
            )
            .bind(item_id)
            .bind(m.path.to_string_lossy().to_string())
            .bind(season)
            .bind(episode)
            .bind(episode_end)
            .bind(m.size)
            .bind(m.mtime)
            .bind(ts)
            .fetch_one(&mut *tx)
            .await?;

            if inserted {
                new_media += 1;
            }

            keep_media.push(id);
        }

        let ids = keep_media.iter().map(i64::to_string).collect::<Vec<_>>().join(",");

        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM media WHERE item_id = ? AND id NOT IN ({ids})")))
            .bind(item_id)
            .execute(&mut *tx)
            .await?;
    }

    let ids = keep_items.iter().map(i64::to_string).collect::<Vec<_>>().join(",");

    let removed = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM items WHERE library = ? AND id NOT IN ({})",
        if ids.is_empty() { "-1".to_string() } else { ids }
    )))
    .bind(name)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let previous: HashMap<String, String> =
        sqlx::query_as::<_, (String, String)>("SELECT path, reason FROM skipped WHERE library = ?")
            .bind(name)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();

    sqlx::query("DELETE FROM skipped WHERE library = ?").bind(name).execute(&mut *tx).await?;

    for (path, reason) in &walk.skipped {
        let path = path.to_string_lossy().to_string();

        if previous.get(&path) == Some(reason) {
            tracing::debug!("skipped {path}: {reason}");
        } else {
            tracing::warn!("skipped {path}: {reason}");
        }

        sqlx::query("INSERT OR REPLACE INTO skipped (path, library, reason, seen_at) VALUES (?, ?, ?, ?)")
            .bind(&path)
            .bind(name)
            .bind(reason)
            .bind(ts)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    let shows = walk.items.iter().filter(|i| i.kind == Kind::Show).count();
    let movies = walk.items.len() - shows;

    tracing::info!(
        "scanned {name:?} in {:.1?}: {shows} shows, {movies} movies, {media_count} files ({new_media} new, {removed} titles removed), {} skipped",
        started.elapsed(),
        walk.skipped.len(),
    );

    state.events.send(Event::ScanFinished {
        library: name.to_string(),
        items: walk.items.len(),
        media: media_count,
        skipped: walk.skipped.len(),
    });

    if new_media > 0 || removed > 0 {
        state.events.send(Event::LibraryChanged { library: name.to_string() });
    }

    state.metadata.wake();
    crate::media::probe::fill_missing_durations(state.clone(), name.to_string());
    Ok(())
}

async fn prune_removed_libraries(state: &AppState, names: &[String]) -> anyhow::Result<()> {
    let placeholders = vec!["?"; names.len().max(1)].join(",");
    let sql = format!("DELETE FROM items WHERE library NOT IN ({placeholders})");
    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.clone()));
    let skipped_sql = sql.replace("items", "skipped");
    let mut q2 = sqlx::query(sqlx::AssertSqlSafe(skipped_sql));

    if names.is_empty() {
        q = q.bind("");
        q2 = q2.bind("");
    }

    for n in names {
        q = q.bind(n);
        q2 = q2.bind(n);
    }

    let removed = q.execute(&state.db).await?.rows_affected();
    q2.execute(&state.db).await?;

    for table in ["tracks", "albums", "artists"] {
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.replace("items", table)));

        if names.is_empty() {
            q = q.bind("");
        }

        for n in names {
            q = q.bind(n);
        }

        q.execute(&state.db).await?;
    }

    if removed > 0 {
        tracing::info!("forgot {removed} titles from libraries removed from config.toml");
    }

    Ok(())
}

pub fn spawn_triggers(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut rx = state.config.subscribe();
        let mut previous: Vec<(String, String, bool)> = Vec::new();
        let mut watchers: Option<notify::RecommendedWatcher> = None;
        let (fs_tx, mut fs_rx) = mpsc::unbounded_channel::<PathBuf>();
        let mut interval_task: Option<tokio::task::JoinHandle<()>> = None;
        let mut previous_interval = None;

        loop {
            let config = rx.borrow_and_update().clone();
            let config_dir = state.config.config_dir().to_path_buf();

            let current: Vec<(String, String, bool)> =
                config.libraries.iter().map(|l| (l.name.clone(), l.path.clone(), config.scan.watch)).collect();

            if current != previous {
                let names: Vec<String> = config.libraries.iter().map(|l| l.name.clone()).collect();

                if let Err(e) = prune_removed_libraries(&state, &names).await {
                    tracing::error!("{e:#}");
                }

                for lib in &config.libraries {
                    let unchanged = previous.iter().any(|(n, p, _)| n == &lib.name && p == &lib.path);

                    if !unchanged || previous.is_empty() {
                        state.scanner.request(&lib.name);
                    }
                }

                watchers = None;

                if config.scan.watch {
                    watchers = watch_libraries(&config, &config_dir, fs_tx.clone());
                }

                previous = current;
            }

            if previous_interval != Some(config.scan.interval) {
                if let Some(t) = interval_task.take() {
                    t.abort();
                }

                if let Some(every) = config.scan.interval.filter(|d| !d.is_zero()) {
                    let state = state.clone();

                    interval_task = Some(tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(*every).await;

                            for lib in &state.config.current().libraries {
                                state.scanner.request(&lib.name);
                            }
                        }
                    }));
                }

                previous_interval = Some(config.scan.interval);
            }

            tokio::select! {
                changed = rx.changed() => if changed.is_err() { break },
                Some(path) = fs_rx.recv() => {

                    let mut paths = vec![path];
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    while let Ok(p) = fs_rx.try_recv() { paths.push(p); }
                    let config = state.config.current();

                    let download_dirs = download_dirs(&state, &config);

                    paths.retain(|p| {
                        let p = p.canonicalize().unwrap_or_else(|_| p.clone());
                        !download_dirs.iter().any(|d| p.starts_with(d))
                    });

                    let mut hit = HashSet::new();

                    for lib in &config.libraries {
                        if let Ok(root) = lib.resolved_path(&config_dir)
                            && paths.iter().any(|p| p.starts_with(&root))
                        {
                            hit.insert(lib.name.clone());
                        }
                    }

                    for name in hit {
                        tracing::debug!("files changed in {name:?}; rescanning");
                        state.scanner.request(&name);
                    }
                }
            }
        }

        drop(watchers);
    });
}

fn watch_libraries(
    config: &crate::config::Config,
    config_dir: &Path,
    tx: mpsc::UnboundedSender<PathBuf>,
) -> Option<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res
            && !matches!(event.kind, notify::EventKind::Access(_))
        {
            for p in event.paths {
                let _ = tx.send(p);
            }
        }
    })
    .map_err(|e| tracing::warn!("can't watch libraries for changes: {e}"))
    .ok()?;

    for lib in &config.libraries {
        let Ok(root) = lib.resolved_path(config_dir) else { continue };

        if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
            tracing::warn!(
                "can't watch {} for changes ({e}); use `[scan] interval` to rescan periodically",
                root.display()
            );
        }
    }

    Some(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Library(PathBuf);

    impl Library {
        fn new(files: &[&str]) -> Self {
            let root = std::env::temp_dir().join(format!("tinystream-scan-{}", uuid::Uuid::new_v4()));

            for file in files {
                let path = root.join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, b"video").unwrap();
            }

            Self(root)
        }

        fn scan(&self) -> Walk {
            walk(&self.0, &[]).unwrap()
        }
    }

    impl Drop for Library {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn season_extras_do_not_collide_with_episodes() {
        let library = Library::new(&[
            "Show/Season 01/Show S01E01.mkv",
            "Show/Season 01/extra/Show S01E01 Director's cut.mkv",
            "Show/Season 01/extras/OP.mp4",
            "Show/Season 01/extras/credits/ED.mkv",
            "Show/Season 01/extras/.hidden.mkv",
            "Show/Season 01/extras/sample.mkv",
            "Show/Season 01/extras/OP.ass",
            "Show/Season 02/Show S02E01.mkv",
        ]);

        let scan = library.scan();
        assert_eq!(scan.items.len(), 1);
        let media = &scan.items[0].media;
        assert_eq!(media.len(), 5);
        assert_eq!(media.iter().filter(|m| m.ep.is_some()).count(), 2);
        assert_eq!(media.iter().filter(|m| m.ep.is_none() && m.season == Some(1)).count(), 3);
        assert_eq!(scan.skipped.len(), 1);
        let again = library.scan();

        assert_eq!(
            media.iter().map(|m| &m.path).collect::<Vec<_>>(),
            again.items[0].media.iter().map(|m| &m.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn numbered_and_unnumbered_specials() {
        for folder in ["S00", "Season 00", "Special", "Specials"] {
            let library = Library::new(&[&format!("Show/{folder}/Show S00E01.mkv"), &format!("Show/{folder}/OVA.mkv")]);
            let scan = library.scan();
            let media = &scan.items[0].media;
            assert_eq!(media.len(), 2);
            assert!(media.iter().all(|m| m.season == Some(0)));
            assert_eq!(media.iter().filter(|m| m.ep.is_none()).count(), 1);
        }
    }

    #[test]
    fn ordinary_unnumbered_videos_are_still_skipped() {
        let library = Library::new(&["Show/S01/Show S01E01.mkv", "Show/S01/Unknown.mkv", "Movie (2024)/Movie.mkv"]);
        let scan = library.scan();
        assert_eq!(scan.items.len(), 2);
        let show = scan.items.iter().find(|i| i.kind == Kind::Show).unwrap();
        assert_eq!(show.media.len(), 1);
        let movie = scan.items.iter().find(|i| i.kind == Kind::Movie).unwrap();
        assert!(movie.media[0].season.is_none());
        assert_eq!(scan.skipped.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn extras_do_not_follow_directory_symlink_cycles() {
        let library = Library::new(&["Show/S01/extras/OP.mkv"]);
        let extras = library.0.join("Show/S01/extras");
        std::os::unix::fs::symlink(&extras, extras.join("loop")).unwrap();
        assert_eq!(library.scan().items[0].media.len(), 1);
    }
}
