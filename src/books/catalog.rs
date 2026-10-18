// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::db::now;

pub const COLUMNS: &str = "id, library, path, size, mtime, format, title, sort_title, authors, series, series_index, \
    description, publisher, published, language, isbn, genres, length, chapters, embedded_cover, provider, \
    provider_id, overview, subjects, online_cover, online_date, rating, pages, fetched_at, added_at";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BookRow {
    pub id: i64,
    pub library: String,
    pub path: String,
    pub size: i64,
    pub mtime: i64,
    pub format: String,
    pub title: String,
    pub sort_title: String,
    pub authors: String,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub published: Option<String>,
    pub language: Option<String>,
    pub isbn: Option<String>,
    pub genres: String,
    pub length: i64,
    pub chapters: i64,
    pub embedded_cover: bool,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub overview: Option<String>,
    pub subjects: String,
    pub online_cover: Option<String>,
    pub online_date: Option<String>,
    pub rating: Option<f64>,
    pub pages: Option<i64>,
    pub fetched_at: Option<i64>,
    pub added_at: i64,
}

pub fn list(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

impl BookRow {
    pub fn authors(&self) -> Vec<String> {
        list(&self.authors)
    }

    /// Its genres: the file's, or else what was found online.
    pub fn genres(&self) -> Vec<String> {
        let own = list(&self.genres);
        if own.is_empty() { list(&self.subjects) } else { own }
    }

    pub fn has_cover(&self) -> bool {
        self.embedded_cover || self.online_cover.is_some()
    }
}

#[derive(Debug, Clone, Copy, Default, sqlx::FromRow)]
pub struct Reading {
    pub position: i64,
    pub length: i64,
    pub finished: bool,
    pub updated_at: i64,
}

impl Reading {
    /// How far into the book this is, from 0 to 1.
    pub fn progress(&self) -> f64 {
        if self.finished {
            return 1.0;
        }

        if self.length <= 0 {
            return 0.0;
        }

        (self.position as f64 / self.length as f64).clamp(0.0, 1.0)
    }

    /// The position, in the book as it is now (which may have been changed since).
    pub fn position_in(&self, length: i64) -> i64 {
        if self.length == length || self.length <= 0 {
            return self.position.min(length);
        }

        ((self.position as f64 / self.length as f64) * length as f64).round() as i64
    }
}

pub async fn book(db: &SqlitePool, id: i64) -> sqlx::Result<Option<BookRow>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!("SELECT {COLUMNS} FROM books WHERE id = ?")))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn books(db: &SqlitePool, library: &str) -> sqlx::Result<Vec<BookRow>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM books WHERE library = ?
         ORDER BY lower(COALESCE(series, sort_title)), series_index, sort_title"
    )))
    .bind(library)
    .fetch_all(db)
    .await
}

pub async fn recently_added(db: &SqlitePool, library: &str, limit: i64) -> sqlx::Result<Vec<BookRow>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM books WHERE library = ? ORDER BY added_at DESC, id DESC LIMIT ?"
    )))
    .bind(library)
    .bind(limit)
    .fetch_all(db)
    .await
}

/// Books someone is part of the way through, the latest first.
pub async fn reading_now(db: &SqlitePool, user_id: i64, limit: i64) -> sqlx::Result<Vec<BookRow>> {
    let columns = COLUMNS.split(", ").map(|c| format!("b.{}", c.trim())).collect::<Vec<_>>().join(", ");

    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {columns} FROM reading r JOIN books b ON b.path = r.book_path
         WHERE r.user_id = ? AND r.finished = 0 AND r.position > 0
         ORDER BY r.updated_at DESC LIMIT ?"
    )))
    .bind(user_id)
    .bind(limit)
    .fetch_all(db)
    .await
}

pub async fn reading(db: &SqlitePool, user_id: i64, paths: &[&str]) -> sqlx::Result<HashMap<String, Reading>> {
    let mut out = HashMap::new();

    for chunk in paths.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT book_path, position, length, finished, updated_at FROM reading WHERE user_id = ? AND book_path IN ({marks})"
        );
        let mut q = sqlx::query_as::<_, (String, i64, i64, bool, i64)>(sqlx::AssertSqlSafe(sql)).bind(user_id);

        for p in chunk {
            q = q.bind(*p);
        }

        for (path, position, length, finished, updated_at) in q.fetch_all(db).await? {
            out.insert(path, Reading { position, length, finished, updated_at });
        }
    }

    Ok(out)
}

pub async fn save_reading(
    db: &SqlitePool,
    user_id: i64,
    book: &BookRow,
    position: i64,
    finished: Option<bool>,
) -> sqlx::Result<Reading> {
    let position = position.clamp(0, book.length);
    let finished = finished.unwrap_or(book.length > 0 && position >= book.length);
    let ts = now();

    sqlx::query(
        "INSERT INTO reading (user_id, book_path, position, length, finished, updated_at) VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, book_path) DO UPDATE SET
            position = excluded.position, length = excluded.length, finished = excluded.finished,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(&book.path)
    .bind(position)
    .bind(book.length)
    .bind(finished)
    .bind(ts)
    .execute(db)
    .await?;

    Ok(Reading { position, length: book.length, finished, updated_at: ts })
}

pub async fn forget_reading(db: &SqlitePool, user_id: i64, path: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM reading WHERE user_id = ? AND book_path = ?").bind(user_id).bind(path).execute(db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Reading;

    #[test]
    fn progress_follows_the_book_as_it_is() {
        let r = Reading { position: 500, length: 1000, finished: false, updated_at: 0 };
        assert_eq!(r.progress(), 0.5);
        assert_eq!(r.position_in(2000), 1000);
        assert_eq!(Reading { finished: true, ..r }.progress(), 1.0);
    }
}
