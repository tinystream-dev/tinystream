// SPDX-License-Identifier: AGPL-3.0-or-later

//! # tinystream
//!
//! <img src="https://raw.githubusercontent.com/tinystream-dev/tinystream/main/assets/logos/logo.svg" alt="tinystream logo" width="128">
//!
//! A small self-hosted media server for your shows, movies and music.
//!
//! [![CI](https://github.com/tinystream-dev/tinystream/actions/workflows/ci.yml/badge.svg)](https://github.com/tinystream-dev/tinystream/actions/workflows/ci.yml)
//! [![Release](https://github.com/tinystream-dev/tinystream/actions/workflows/release.yml/badge.svg)](https://github.com/tinystream-dev/tinystream/actions/workflows/release.yml)
//! [![Latest release](https://img.shields.io/github/v/release/tinystream-dev/tinystream?sort=semver)](https://github.com/tinystream-dev/tinystream/releases/latest)
//! [![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](https://github.com/tinystream-dev/tinystream/blob/main/LICENSE)
//! [![Rust 1.94+](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org)
//!
//! ## License
//!
//! Released under the [AGPL-3.0-or-later](https://github.com/tinystream-dev/tinystream/blob/main/LICENSE) open-source license.

#![cfg_attr(not(feature = "torrent"), allow(dead_code, unused_imports))]
#![recursion_limit = "256"]

mod api;
mod auth;
#[cfg(feature = "torrent")]
mod automation;
mod books;
mod cli;
mod clips;
mod config;
mod db;
mod error;
mod events;
mod library;
mod logging;
mod mail;
mod media;
mod metadata;
mod music;
mod notifications;
mod paths;
mod state;
mod theme;
mod tint;
mod together;
#[cfg(feature = "web-ui")]
mod web;

use std::sync::Arc;

use anyhow::Context;
use axum::serve::ListenerExt;
use clap::Parser;
use tokio::net::TcpListener;

use crate::cli::{Cli, Command};
use crate::config::{Config, ConfigStore};
use crate::events::Events;
use crate::paths::Paths;
use crate::state::AppState;

fn main() -> std::process::ExitCode {
    #[cfg(feature = "torrent")]
    automation::init_local_offset();

    let cli = Cli::parse();

    let paths = match Paths::resolve(&cli) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e:#}");
            return std::process::ExitCode::FAILURE;
        },
    };

    match cli.command.unwrap_or(Command::Serve) {
        Command::Paths => {
            print_paths(&paths);
            std::process::ExitCode::SUCCESS
        },
        Command::Check => check(&paths),
        Command::ResetPassword { username, password } => run_async(reset_password(paths, username, password)),
        Command::Serve => run_async(serve(paths)),
    }
}

fn run_async(f: impl Future<Output = anyhow::Result<()>>) -> std::process::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    match rt.block_on(f) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e:#}");
            eprintln!("error: {e:#}");
            std::process::ExitCode::FAILURE
        },
    }
}

fn print_paths(paths: &Paths) {
    println!("config    {}  ({})", paths.config_file.display(), paths.config_source);
    println!("data      {}  ({})", paths.data_dir.display(), paths.data_source);
    println!("database  {}", paths.database().display());
    println!("logs      {}, {}", paths.current_log().display(), paths.previous_log().display());

    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() => println!("$HOME     {h}"),
        _ => println!("$HOME     (not set: `~` in config.toml won't work)"),
    }
}

fn check(paths: &Paths) -> std::process::ExitCode {
    let text = match std::fs::read_to_string(&paths.config_file) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("can't read {}: {e}", paths.config_file.display());
            return std::process::ExitCode::FAILURE;
        },
    };

    match Config::parse(&text) {
        Ok(config) => {
            println!("{} is valid.", paths.config_file.display());
            let mut ok = true;

            for lib in &config.libraries {
                match lib.resolved_path(paths.config_dir()) {
                    Ok(p) if p.is_dir() => println!("  ✓ {} → {}", lib.name, p.display()),
                    Ok(p) => {
                        ok = false;
                        println!("  ✗ {} → {} (not a folder)", lib.name, p.display())
                    },
                    Err(e) => {
                        ok = false;
                        println!("  ✗ {}: {e:#}", lib.name)
                    },
                }
            }

            if ok { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
        },
        Err(e) => {
            eprintln!("{} is invalid:\n{e}", paths.config_file.display());
            std::process::ExitCode::FAILURE
        },
    }
}

async fn reset_password(paths: Paths, username: String, password: Option<String>) -> anyhow::Result<()> {
    let password = match password {
        Some(p) => p,
        None => {
            eprint!("New password for {username}: ");
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)?;
            line.trim_end_matches(['\r', '\n']).to_string()
        },
    };

    auth::validate_credentials(&username, &password).map_err(|e| anyhow::anyhow!(e.message))?;
    let db = db::open(&paths.database()).await?;
    let hash = auth::hash_password(password).await?;

    let updated = sqlx::query("UPDATE users SET password_hash = ? WHERE username = ?")
        .bind(hash)
        .bind(&username)
        .execute(&db)
        .await?
        .rows_affected();

    anyhow::ensure!(updated == 1, "there's no account called {username:?}");

    sqlx::query("DELETE FROM sessions WHERE user_id = (SELECT id FROM users WHERE username = ?)")
        .bind(&username)
        .execute(&db)
        .await?;

    println!("Password for {username} changed; they've been signed out everywhere.");
    Ok(())
}

async fn serve(paths: Paths) -> anyhow::Result<()> {
    let level = std::fs::read_to_string(&paths.config_file)
        .ok()
        .and_then(|t| Config::parse(&t).ok())
        .map(|c| c.log.level)
        .unwrap_or_else(|| "info".into());

    let log = logging::init(&paths, &level)?;
    tracing::info!("tinystream {}", env!("TINYSTREAM_VERSION"));
    tracing::info!("config: {} ({})", paths.config_file.display(), paths.config_source);
    tracing::info!("data:   {} ({})", paths.data_dir.display(), paths.data_source);

    let events = Events::new();
    let config = ConfigStore::load(&paths.config_file, events.clone())?;
    config.watch().context("can't watch config.toml for changes")?;
    let db = db::open(&paths.database()).await.context("can't open the database")?;

    let http = reqwest::Client::builder()
        .user_agent(concat!("tinystream/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let (scanner, scan_rx) = library::Scanner::new();
    let media = Arc::new(media::MediaService::new(paths.cache_dir()));
    media.hw.configure(&config.current().transcode);

    #[cfg(feature = "torrent")]
    let automation = Arc::new(automation::Automation::new(&config.current())?);

    let state = Arc::new(AppState {
        #[cfg(feature = "torrent")]
        automation,
        paths,
        config,
        db,
        events,
        log,
        scanner,
        metadata: metadata::MetadataService::new(http.clone()),
        media,
        passkeys: Default::default(),
        http,
        together: Default::default(),
        clips: Default::default(),
        music: Default::default(),
        books: Default::default(),
        tints: Default::default(),
    });

    library::scanner::spawn_worker(state.clone(), scan_rx);
    library::scanner::spawn_triggers(state.clone());
    metadata::spawn_worker(state.clone());
    music::spawn(state.clone());
    books::spawn(state.clone());

    #[cfg(feature = "torrent")]
    automation::spawn(state.clone());

    spawn_config_followers(state.clone());
    state.together.clone().spawn(state.db.clone());
    state.clips.start(&state).await;

    let mut rx = state.config.subscribe();

    loop {
        let network = rx.borrow_and_update().network.clone();
        let addr = format!("{}:{}", network.host, network.port);

        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("can't listen on {addr}: {e}. Change [network] in config.toml; I'll pick it up");

                tokio::select! {
                    r = rx.changed() => { r?; continue }
                    _ = shutdown_signal() => return Ok(()),
                }
            },
        };

        let shown = if network.host == "0.0.0.0" || network.host == "::" { "localhost" } else { &network.host };
        tracing::info!("ready at http://{shown}:{}", network.port);

        let app = api::router(state.clone());

        tokio::select! {
            r = axum::serve(
                listener.tap_io(|tcp| { let _ = tcp.set_nodelay(true); }),
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            ) => r?,
            _ = network_changed(&mut rx, &network) => {
                tracing::info!("[network] changed; moving to the new address");
            }
            _ = shutdown_signal() => {
                tracing::info!("shutting down");

                #[cfg(feature = "torrent")]
                automation::engine::flush(&state).await;

                return Ok(());
            }
        }
    }
}

async fn network_changed(rx: &mut tokio::sync::watch::Receiver<Arc<Config>>, current: &config::Network) {
    loop {
        if rx.changed().await.is_err() {
            return std::future::pending().await;
        }

        let n = &rx.borrow().network;

        if n.host != current.host || n.port != current.port {
            return;
        }
    }
}

fn spawn_config_followers(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut rx = state.config.subscribe();
        let mut previous = rx.borrow_and_update().clone();

        while rx.changed().await.is_ok() {
            let config = rx.borrow_and_update().clone();

            if config.log != previous.log {
                state.log.set_level(&config.log.level);
            }

            if config.transcode != previous.transcode {
                state.media.hw.configure(&config.transcode);
            }

            if config.clips != previous.clips {
                if config.clips.enabled {
                    state.clips.resume(&state).await;
                } else {
                    state.clips.pause_all();
                }
            }

            if config.metadata != previous.metadata {
                state.metadata.wake();
            }

            if config.music != previous.music {
                music::loudness::wake(&state);
            }

            previous = config;
        }
    });
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }

    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}
