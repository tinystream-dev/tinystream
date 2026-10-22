// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::db::now;
use crate::library::parse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Transfer {
    Hardlink,
    Copy,
    Move,
}

impl Transfer {
    pub fn as_str(self) -> &'static str {
        match self {
            Transfer::Hardlink => "hardlink",
            Transfer::Copy => "copy",
            Transfer::Move => "move",
        }
    }
}

pub struct Batch<'a> {
    db: &'a SqlitePool,
    pub id: String,
    label: String,
}

fn part_path(dst: &Path) -> PathBuf {
    let name = dst.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    dst.with_file_name(format!(".{name}.tinystream-part"))
}

impl<'a> Batch<'a> {
    pub fn new(db: &'a SqlitePool, id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { db, id: id.into(), label: label.into() }
    }

    async fn record(&self, kind: &str, src: Option<&Path>, dst: &Path) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO file_ops (batch, label, kind, src, dst, at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&self.id)
            .bind(&self.label)
            .bind(kind)
            .bind(src.map(|p| p.to_string_lossy().to_string()))
            .bind(dst.to_string_lossy().to_string())
            .bind(now())
            .execute(self.db)
            .await?;

        Ok(())
    }

    pub async fn mkdirs(&self, dir: &Path) -> anyhow::Result<()> {
        let mut missing = Vec::new();
        let mut at = Some(dir);

        while let Some(d) = at {
            if d.exists() {
                break;
            }

            missing.push(d.to_path_buf());
            at = d.parent();
        }

        for d in missing.into_iter().rev() {
            std::fs::create_dir(&d).with_context(|| format!("can't create {}", d.display()))?;
            self.record("mkdir", None, &d).await?;
        }

        Ok(())
    }

    pub async fn rename(&self, src: &Path, dst: &Path) -> anyhow::Result<()> {
        if src == dst {
            return Ok(());
        }

        let same_file = dst.exists() && same_inode(src, dst);

        if dst.exists() && !same_file {
            bail!("{} already exists; not overwriting it", dst.display());
        }

        if let Some(parent) = dst.parent() {
            self.mkdirs(parent).await?;
        }

        move_file(src, dst).await?;
        self.record("rename", Some(src), dst).await
    }

    pub async fn transfer(&self, src: &Path, dst: &Path, how: &[Transfer]) -> anyhow::Result<Transfer> {
        if dst.exists() {
            bail!("{} already exists; not overwriting it", dst.display());
        }

        if let Some(parent) = dst.parent() {
            self.mkdirs(parent).await?;
        }

        let mut last_error = None;

        for &t in how {
            let result = match t {
                Transfer::Hardlink => std::fs::hard_link(src, dst).map_err(anyhow::Error::from),
                Transfer::Copy => copy_into_place(src, dst).await,
                Transfer::Move => move_file(src, dst).await,
            };

            match result {
                Ok(()) => {
                    self.record(t.as_str(), Some(src), dst).await?;
                    return Ok(t);
                },
                Err(e) => {
                    tracing::debug!("{} {} → {} didn't work: {e:#}", t.as_str(), src.display(), dst.display());
                    last_error = Some(e);
                },
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no way to import was allowed")))
            .with_context(|| format!("can't put {} into the library", src.display()))
    }
}

fn same_inode(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.ino() == b.ino() && a.dev() == b.dev(),
        _ => false,
    }
}

async fn copy_into_place(src: &Path, dst: &Path) -> anyhow::Result<()> {
    let part = part_path(dst);
    let (s, p) = (src.to_path_buf(), part.clone());

    let copied = tokio::task::spawn_blocking(move || std::fs::copy(&s, &p)).await?;

    if let Err(e) = copied {
        let _ = std::fs::remove_file(&part);
        return Err(e.into());
    }

    if dst.exists() {
        let _ = std::fs::remove_file(&part);
        bail!("{} appeared while copying; not overwriting it", dst.display());
    }

    std::fs::rename(&part, dst)?;
    Ok(())
}

async fn move_file(src: &Path, dst: &Path) -> anyhow::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),

        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            copy_into_place(src, dst).await?;
            std::fs::remove_file(src)?;
            Ok(())
        },
        Err(e) => Err(anyhow::Error::from(e).context(format!("can't move {} to {}", src.display(), dst.display()))),
    }
}

#[derive(Debug, Default, Serialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport {
    pub undone: usize,
    pub problems: Vec<String>,
}

pub async fn undo(db: &SqlitePool, batch: &str) -> anyhow::Result<UndoReport> {
    let ops: Vec<(i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, kind, src, dst FROM file_ops WHERE batch = ? AND undone_at IS NULL ORDER BY id DESC",
    )
    .bind(batch)
    .fetch_all(db)
    .await?;

    let mut report = UndoReport::default();

    for (id, kind, src, dst) in ops {
        let dst = PathBuf::from(dst);

        let result: anyhow::Result<()> = async {
            match kind.as_str() {
                "rename" | "move" => {
                    let src = PathBuf::from(src.context("no source recorded")?);

                    if src.exists() {
                        bail!("{} exists again; leaving {} where it is", src.display(), dst.display());
                    }

                    if !dst.exists() {
                        bail!("{} is gone", dst.display());
                    }

                    if let Some(parent) = src.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    move_file(&dst, &src).await
                },
                "hardlink" | "copy" => {
                    if dst.exists() {
                        std::fs::remove_file(&dst)?;
                    }
                    Ok(())
                },
                "mkdir" => {
                    let _ = std::fs::remove_dir(&dst);
                    Ok(())
                },
                other => bail!("don't know how to undo {other:?}"),
            }
        }
        .await;

        match result {
            Ok(()) => {
                report.undone += 1;
                sqlx::query("UPDATE file_ops SET undone_at = ? WHERE id = ?").bind(now()).bind(id).execute(db).await?;
            },
            Err(e) => report.problems.push(format!("{e:#}")),
        }
    }

    Ok(report)
}

/// Where `path` is now, following any renames made after it was put there.
async fn current_path(db: &SqlitePool, path: String) -> anyhow::Result<String> {
    let mut at = path;

    for _ in 0..32 {
        let next: Option<String> = sqlx::query_scalar(
            "SELECT dst FROM file_ops WHERE kind = 'rename' AND src = ? AND undone_at IS NULL ORDER BY id DESC LIMIT 1",
        )
        .bind(&at)
        .fetch_optional(db)
        .await?;

        match next {
            Some(n) => at = n,
            None => break,
        }
    }

    Ok(at)
}

/// Which season a library file is, from the last scan or else its name.
async fn season_of(db: &SqlitePool, path: &Path) -> anyhow::Result<Option<u32>> {
    let scanned: Option<Option<i64>> = sqlx::query_scalar("SELECT season FROM media WHERE path = ?")
        .bind(path.to_string_lossy().to_string())
        .fetch_optional(db)
        .await?;

    if let Some(Some(season)) = scanned {
        return Ok(Some(season as u32));
    }

    let named = path.file_stem().and_then(|s| parse::episode_number(&s.to_string_lossy())).map(|n| n.season);

    Ok(named.or_else(|| {
        let folder = path.parent()?.file_name()?.to_string_lossy().to_string();
        parse::season_number(&folder)
    }))
}

/// Deletes the files a batch put into the library (only those of `season`,
/// when given), and the folders it made for them once they're empty. Unlike
/// [`undo`], moved files aren't put back.
pub async fn delete_placed(db: &SqlitePool, batch: &str, season: Option<u32>) -> anyhow::Result<UndoReport> {
    let ops: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, kind, dst FROM file_ops WHERE batch = ? AND undone_at IS NULL ORDER BY id DESC")
            .bind(batch)
            .fetch_all(db)
            .await?;

    let mut report = UndoReport::default();

    for (id, kind, dst) in ops {
        let result: anyhow::Result<()> = match kind.as_str() {
            "hardlink" | "copy" | "move" => {
                let at = PathBuf::from(current_path(db, dst).await?);

                if season.is_some() && season_of(db, &at).await? != season {
                    continue;
                }

                match std::fs::remove_file(&at) {
                    Err(e) if e.kind() != ErrorKind::NotFound => {
                        Err(anyhow::Error::from(e).context(format!("can't delete {}", at.display())))
                    },
                    _ => {
                        sqlx::query("DELETE FROM media WHERE path = ?")
                            .bind(at.to_string_lossy().to_string())
                            .execute(db)
                            .await?;

                        Ok(())
                    },
                }
            },
            "mkdir" => {
                let _ = std::fs::remove_dir(&dst);

                if Path::new(&dst).exists() {
                    continue;
                }

                Ok(())
            },
            _ => continue,
        };

        match result {
            Ok(()) => {
                report.undone += 1;
                sqlx::query("UPDATE file_ops SET undone_at = ? WHERE id = ?").bind(now()).bind(id).execute(db).await?;
            },
            Err(e) => report.problems.push(format!("{e:#}")),
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn db() -> SqlitePool {
        let dir = std::env::temp_dir().join(format!("ts-fsops-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        crate::db::open(&dir.join("t.db")).await.unwrap()
    }

    #[tokio::test]
    async fn rename_and_undo() {
        let db = db().await;
        let dir = std::env::temp_dir().join(format!("ts-fsops-files-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.mkv");
        std::fs::write(&a, b"x").unwrap();
        let b = dir.join("Season 01/b.mkv");
        let batch = Batch::new(&db, "t1", "test");
        batch.rename(&a, &b).await.unwrap();
        assert!(b.exists() && !a.exists());

        std::fs::write(&a, b"y").unwrap();
        assert!(batch.rename(&a, &b).await.is_err());
        std::fs::remove_file(&a).unwrap();

        let report = undo(&db, "t1").await.unwrap();
        assert_eq!(report.undone, 2, "{:?}", report.problems);
        assert!(a.exists() && !dir.join("Season 01").exists());
    }

    #[tokio::test]
    async fn transfer_falls_back() {
        let db = db().await;
        let dir = std::env::temp_dir().join(format!("ts-fsops-files-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("dl.mkv");
        std::fs::write(&src, b"data").unwrap();
        let batch = Batch::new(&db, "t2", "test");
        let how = batch.transfer(&src, &dir.join("lib/ep.mkv"), &[Transfer::Hardlink, Transfer::Copy]).await.unwrap();
        assert_eq!(how, Transfer::Hardlink);
        assert!(src.exists());
        assert!(batch.transfer(&src, &dir.join("lib/ep.mkv"), &[Transfer::Copy]).await.is_err());
    }

    #[tokio::test]
    async fn delete_placed_follows_renames() {
        let db = db().await;
        let dir = std::env::temp_dir().join(format!("ts-fsops-files-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("dl.mkv");
        std::fs::write(&src, b"data").unwrap();
        let placed = dir.join("lib/Season 01/ep.mkv");
        Batch::new(&db, "import-1", "test").transfer(&src, &placed, &[Transfer::Move]).await.unwrap();
        let renamed = dir.join("lib/Season 01/S01E01.mkv");
        Batch::new(&db, "rename-1", "test").rename(&placed, &renamed).await.unwrap();

        let report = delete_placed(&db, "import-1", None).await.unwrap();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert!(!renamed.exists() && !src.exists());
        assert!(!dir.join("lib").exists());
    }

    #[tokio::test]
    async fn delete_placed_keeps_other_seasons() {
        let db = db().await;
        let dir = std::env::temp_dir().join(format!("ts-fsops-files-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let batch = Batch::new(&db, "import-2", "test");

        for (src, dst) in [("a.mkv", "lib/Season 01/Show S01E01.mkv"), ("b.mkv", "lib/Season 02/Show S02E01.mkv")] {
            std::fs::write(dir.join(src), b"x").unwrap();
            batch.transfer(&dir.join(src), &dir.join(dst), &[Transfer::Move]).await.unwrap();
        }

        let report = delete_placed(&db, "import-2", Some(2)).await.unwrap();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert!(dir.join("lib/Season 01/Show S01E01.mkv").exists());
        assert!(!dir.join("lib/Season 02").exists());
    }
}
