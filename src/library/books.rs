// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::books::{Book, Format};
use crate::db::now;
use crate::events::Event;
use crate::state::AppState;

struct Found {
    path: PathBuf,
    size: i64,
    mtime: i64,
}

fn walk(root: &Path, download_dirs: &[PathBuf], out: &mut Vec<Found>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        tracing::warn!("can't read {} (permissions?)", root.display());
        return;
    };

    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }

        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };

        if ft.is_dir() || (ft.is_symlink() && path.is_dir()) {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

            if !download_dirs.iter().any(|d| d == &canonical) {
                walk(&path, download_dirs, out);
            }
        } else if Format::of(&path).is_some() {
            let (size, mtime) = super::scanner::file_info(&path);
            out.push(Found { path, size, mtime });
        }
    }
}

fn read_all(files: Vec<Found>) -> Vec<(Found, anyhow::Result<Book>)> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunk = files.len().div_ceil(threads).max(1);
    let mut files = files;
    let mut chunks = Vec::new();

    while !files.is_empty() {
        let rest = files.split_off(chunk.min(files.len()));
        chunks.push(std::mem::replace(&mut files, rest));
    }

    std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                s.spawn(move || {
                    chunk
                        .into_iter()
                        .map(|f| {
                            let book = Book::read(&f.path);
                            (f, book)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    })
}

fn json(v: &[String]) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "[]".into())
}

pub async fn scan(state: &AppState, name: &str, root: &Path, download_dirs: Vec<PathBuf>) -> anyhow::Result<()> {
    let started = std::time::Instant::now();

    let found = {
        let root = root.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            walk(&root, &download_dirs, &mut out);
            out
        })
        .await?
    };

    let db = &state.db;

    let known: Vec<(i64, String, i64, i64)> =
        sqlx::query_as("SELECT id, path, size, mtime FROM books WHERE library = ?").bind(name).fetch_all(db).await?;

    if found.is_empty() && !known.is_empty() {
        tracing::warn!(
            "library {name:?}: {} has no books; if it's a drive that isn't mounted, that's why. \
             Keeping the {} books already scanned",
            root.display(),
            known.len()
        );

        return Ok(());
    }

    let unchanged: HashMap<&str, (i64, i64)> = known.iter().map(|(_, p, s, m)| (p.as_str(), (*s, *m))).collect();
    let present: HashSet<String> = found.iter().map(|f| f.path.to_string_lossy().to_string()).collect();

    let (same, changed): (Vec<Found>, Vec<Found>) =
        found.into_iter().partition(|f| unchanged.get(f.path.to_string_lossy().as_ref()) == Some(&(f.size, f.mtime)));

    let read = tokio::task::spawn_blocking(move || read_all(changed)).await?;
    let gone: Vec<i64> = known.iter().filter(|(_, p, ..)| !present.contains(p)).map(|(id, ..)| *id).collect();

    let ts = now();
    let mut tx = db.begin().await?;
    let mut skipped = Vec::new();
    let mut new_books = 0usize;

    for (f, book) in &read {
        let book = match book {
            Ok(b) if b.document.length > 0 => b,
            Ok(_) => {
                skipped.push((f.path.clone(), "there's no text in it".to_string()));
                continue;
            },
            Err(e) => {
                skipped.push((f.path.clone(), format!("can't read it: {e:#}")));
                continue;
            },
        };

        let m = &book.meta;
        let sort_title =
            m.sort_title.as_deref().map(str::to_lowercase).unwrap_or_else(|| super::parse::sort_title(&m.title));

        let inserted: bool = sqlx::query_scalar(
            "INSERT INTO books (library, path, size, mtime, format, title, sort_title, authors, series, series_index,
                description, publisher, published, language, isbn, genres, length, chapters, embedded_cover, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
             ON CONFLICT(path) DO UPDATE SET
                library = excluded.library, size = excluded.size, mtime = excluded.mtime, format = excluded.format,
                fetched_at = CASE WHEN books.title = excluded.title AND books.authors = excluded.authors
                    AND IFNULL(books.isbn, '') = IFNULL(excluded.isbn, '') THEN books.fetched_at ELSE NULL END,
                title = excluded.title, sort_title = excluded.sort_title, authors = excluded.authors,
                series = excluded.series, series_index = excluded.series_index, description = excluded.description,
                publisher = excluded.publisher, published = excluded.published, language = excluded.language,
                isbn = excluded.isbn, genres = excluded.genres, length = excluded.length, chapters = excluded.chapters,
                embedded_cover = excluded.embedded_cover
             RETURNING added_at = ?20",
        )
        .bind(name)
        .bind(f.path.to_string_lossy().to_string())
        .bind(f.size)
        .bind(f.mtime)
        .bind(book.format.as_str())
        .bind(m.title.trim())
        .bind(sort_title)
        .bind(json(&m.authors))
        .bind(&m.series)
        .bind(m.series_index)
        .bind(&m.description)
        .bind(&m.publisher)
        .bind(&m.published)
        .bind(&m.language)
        .bind(&m.isbn)
        .bind(json(&m.genres))
        .bind(book.document.length as i64)
        .bind(book.document.chapters.len() as i64)
        .bind(m.cover.is_some())
        .bind(ts)
        .fetch_one(&mut *tx)
        .await?;

        if inserted {
            new_books += 1;
        }
    }

    for chunk in gone.chunks(500) {
        let ids = chunk.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM books WHERE id IN ({ids})"))).execute(&mut *tx).await?;
    }

    sqlx::query("DELETE FROM skipped WHERE library = ? AND path NOT IN (SELECT path FROM books)")
        .bind(name)
        .execute(&mut *tx)
        .await?;

    for (path, reason) in &skipped {
        tracing::warn!("skipped {}: {reason}", path.display());

        sqlx::query("INSERT OR REPLACE INTO skipped (path, library, reason, seen_at) VALUES (?, ?, ?, ?)")
            .bind(path.to_string_lossy().to_string())
            .bind(name)
            .bind(reason)
            .bind(ts)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    let books: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM books WHERE library = ?").bind(name).fetch_one(db).await?;

    tracing::info!(
        "scanned {name:?} in {:.1?}: {books} books ({} read, {new_books} new, {} removed, {} unchanged), {} skipped",
        started.elapsed(),
        read.len(),
        gone.len(),
        same.len(),
        skipped.len(),
    );

    state.events.send(Event::ScanFinished {
        library: name.to_string(),
        items: books as usize,
        media: books as usize,
        skipped: skipped.len(),
    });

    if !read.is_empty() || !gone.is_empty() {
        state.events.send(Event::LibraryChanged { library: name.to_string() });
    }

    crate::books::online::wake(state);
    Ok(())
}
