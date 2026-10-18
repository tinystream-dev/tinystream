// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;

use async_graphql::{Context, Object, SimpleObject};

use super::schema::Ctx;
use crate::auth::permissions;
use crate::config::{
    Automation, Clips, Config, Downloads, LibraryInput, LibraryKind, Log, Metadata, Music, Network, Profile, Provider,
    Requests, Scan, SettingsPatch, SignIn, Source, Transcode,
};
use crate::error::{ApiError, ApiResult};
use crate::paths::resolve_config_path;
use crate::state::AppState;

pub struct Settings(Arc<Config>);

impl Settings {
    pub(super) fn now(state: &AppState) -> Self {
        Self(state.config.current())
    }
}

#[derive(SimpleObject)]
pub struct ConfiguredLibrary {
    name: String,

    path: String,
    kind: LibraryKind,
    metadata_provider: Option<Provider>,

    managed: bool,
    profile: Option<String>,
    download_path: Option<String>,

    resolved_path: Option<String>,
    exists: bool,

    error: Option<String>,
    title_count: i64,

    skipped_count: i64,
}

#[derive(SimpleObject)]
pub struct ServerPaths {
    config: String,
    data: String,
    log: String,
}

#[Object]
impl Settings {
    async fn network(&self) -> &Network {
        &self.0.network
    }

    async fn log(&self) -> &Log {
        &self.0.log
    }

    async fn scan(&self) -> &Scan {
        &self.0.scan
    }

    async fn metadata(&self) -> &Metadata {
        &self.0.metadata
    }

    async fn transcode(&self) -> &Transcode {
        &self.0.transcode
    }

    async fn clips(&self) -> &Clips {
        &self.0.clips
    }

    async fn music(&self) -> &Music {
        &self.0.music
    }

    async fn downloads(&self) -> &Downloads {
        &self.0.downloads
    }

    async fn automation(&self) -> &Automation {
        &self.0.automation
    }

    async fn requests(&self) -> &Requests {
        &self.0.requests
    }

    async fn sign_in(&self) -> &SignIn {
        &self.0.sign_in
    }

    async fn sources(&self) -> &[Source] {
        &self.0.sources
    }

    async fn profiles(&self) -> &[Profile] {
        &self.0.profiles
    }

    async fn libraries(&self, ctx: &Context<'_>) -> ApiResult<Vec<ConfiguredLibrary>> {
        let state = ctx.state();
        let config_dir = state.config.config_dir();
        let mut out = Vec::new();

        for lib in &self.0.libraries {
            let resolved = lib.resolved_path(config_dir);

            let (title_count, skipped_count): (i64, i64) = sqlx::query_as(
                "SELECT (SELECT COUNT(*) FROM items WHERE library = ?1) + (SELECT COUNT(*) FROM albums WHERE library = ?1)
                        + (SELECT COUNT(*) FROM books WHERE library = ?1),
                        (SELECT COUNT(*) FROM skipped WHERE library = ?1)",
            )
            .bind(&lib.name)
            .fetch_one(&state.db)
            .await?;

            out.push(ConfiguredLibrary {
                name: lib.name.clone(),
                path: lib.path.clone(),
                kind: lib.kind,
                metadata_provider: lib.metadata_provider,
                managed: lib.managed,
                profile: lib.profile.clone(),
                download_path: lib.download_path.clone(),
                resolved_path: resolved.as_ref().ok().map(|p| p.display().to_string()),
                exists: resolved.as_ref().is_ok_and(|p| p.is_dir()),
                error: resolved.as_ref().err().map(|e| format!("{e:#}")),
                title_count,
                skipped_count,
            });
        }

        Ok(out)
    }

    async fn raw(&self, ctx: &Context<'_>) -> String {
        ctx.state().config.raw().unwrap_or_default()
    }

    async fn error(&self, ctx: &Context<'_>) -> Option<String> {
        ctx.state().config.last_error()
    }

    async fn paths(&self, ctx: &Context<'_>) -> ServerPaths {
        let state = ctx.state();

        ServerPaths {
            config: state.config.path().display().to_string(),
            data: state.paths.data_dir.display().to_string(),
            log: state.paths.current_log().display().to_string(),
        }
    }
}

#[derive(SimpleObject)]
pub struct Folder {
    name: String,
    path: String,
}

#[derive(SimpleObject)]
pub struct FolderListing {
    path: String,
    parent: Option<String>,

    home: Option<String>,

    folders: Vec<Folder>,
}

#[derive(SimpleObject)]
pub struct SkippedFile {
    library: String,
    path: String,
    reason: String,
}

#[derive(Default)]
pub struct SettingsQuery;

#[Object]
impl SettingsQuery {
    async fn settings(&self, ctx: &Context<'_>) -> ApiResult<Settings> {
        ctx.admin()?;
        Ok(Settings::now(ctx.state()))
    }

    async fn folders(&self, ctx: &Context<'_>, path: Option<String>) -> ApiResult<FolderListing> {
        let state = ctx.state();
        ctx.admin()?;
        let start = path.filter(|p| !p.trim().is_empty()).unwrap_or_else(|| "~".into());

        let path: PathBuf = resolve_config_path(&start, state.config.config_dir())
            .or_else(|_| resolve_config_path("/", state.config.config_dir()))
            .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;

        let mut folders: Vec<Folder> = std::fs::read_dir(&path)
            .map_err(|e| ApiError::bad_request(format!("can't open {}: {e}", path.display())))?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| Folder { name: e.file_name().to_string_lossy().to_string(), path: e.path().display().to_string() })
            .collect();

        folders.sort_by_key(|f| f.name.to_lowercase());

        Ok(FolderListing {
            parent: path.parent().map(|p| p.display().to_string()),
            path: path.display().to_string(),
            home: std::env::var("HOME").ok(),
            folders,
        })
    }

    async fn skipped_files(&self, ctx: &Context<'_>) -> ApiResult<Vec<SkippedFile>> {
        ctx.admin()?;

        let rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT library, path, reason FROM skipped ORDER BY library, path")
                .fetch_all(&ctx.state().db)
                .await?;

        Ok(rows.into_iter().map(|(library, path, reason)| SkippedFile { library, path, reason }).collect())
    }
}

fn check_library(state: &AppState, input: &LibraryInput) -> ApiResult<()> {
    if input.name.trim().is_empty() {
        return Err(ApiError::bad_request("give the library a name"));
    }

    let path = resolve_config_path(input.path.trim(), state.config.config_dir())
        .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;

    if !path.is_dir() {
        return Err(ApiError::bad_request(format!("{} isn't a folder tinystream can see", path.display())));
    }

    Ok(())
}

fn library_index(state: &AppState, name: &str) -> ApiResult<usize> {
    state.config.current().libraries.iter().position(|l| l.name == name).ok_or_else(|| ApiError::not_found("library"))
}

#[derive(Default)]
pub struct SettingsMutation;

#[Object]
impl SettingsMutation {
    async fn update_settings(&self, ctx: &Context<'_>, patch: SettingsPatch) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        state.config.patch(patch).await.map_err(ApiError::bad_request)?;
        Ok(Settings::now(state))
    }

    async fn replace_config(&self, ctx: &Context<'_>, text: String) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        state.config.replace_raw(text).await.map_err(ApiError::bad_request)?;
        Ok(Settings::now(state))
    }

    async fn add_library(&self, ctx: &Context<'_>, input: LibraryInput) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        check_library(state, &input)?;
        state.config.add_library(input).await.map_err(ApiError::bad_request)?;
        Ok(Settings::now(state))
    }

    async fn update_library(&self, ctx: &Context<'_>, name: String, input: LibraryInput) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        check_library(state, &input)?;
        let index = library_index(state, &name)?;
        let old = state.config.current().libraries[index].clone();
        let new_name = input.name.trim().to_string();

        if old.name != new_name {
            let mut tx = state.db.begin().await?;

            for table in ["items", "skipped", "tracks", "albums", "artists"] {
                sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE {table} SET library = ? WHERE library = ?")))
                    .bind(&new_name)
                    .bind(&old.name)
                    .execute(&mut *tx)
                    .await?;
            }

            for table in ["stars", "ratings"] {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE {table} SET target = ?1 || substr(target, length(?2) + 1)
                     WHERE kind != 'track' AND substr(target, 1, length(?2) + 1) = ?2 || '/'"
                )))
                .bind(&new_name)
                .bind(&old.name)
                .execute(&mut *tx)
                .await?;
            }

            tx.commit().await?;
            permissions::rename_library(state, &old.name, &new_name).await?;
        }

        if old.metadata_provider != input.metadata_provider {
            sqlx::query("UPDATE items SET match_state = 'pending' WHERE library = ? AND match_state != 'manual'")
                .bind(&new_name)
                .execute(&state.db)
                .await?;
        }

        state.config.update_library(index, input).await.map_err(ApiError::bad_request)?;
        state.metadata.wake();
        Ok(Settings::now(state))
    }

    async fn remove_library(&self, ctx: &Context<'_>, name: String) -> ApiResult<Settings> {
        let state = ctx.state();
        ctx.admin()?;
        let index = library_index(state, &name)?;
        state.config.remove_library(index).await.map_err(ApiError::bad_request)?;
        Ok(Settings::now(state))
    }

    async fn scan(&self, ctx: &Context<'_>, library: Option<String>) -> ApiResult<bool> {
        let state = ctx.state();
        ctx.admin()?;
        let config = state.config.current();

        match library {
            Some(name) => {
                config.library(&name).ok_or_else(|| ApiError::not_found("library"))?;
                state.scanner.request(&name);
            },
            None => {
                for lib in &config.libraries {
                    state.scanner.request(&lib.name);
                }
            },
        }

        Ok(true)
    }
}
