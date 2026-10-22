// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs::{self, File};
use std::io::{self, IsTerminal};
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, reload};

use crate::paths::Paths;

#[derive(Clone)]
pub struct LogHandle {
    reload: reload::Handle<EnvFilter, tracing_subscriber::Registry>,
    env_override: bool,
}

impl LogHandle {
    pub fn set_level(&self, level: &str) {
        if self.env_override {
            return;
        }
        match build_filter(level) {
            Ok(filter) => {
                if self.reload.reload(filter).is_ok() {
                    tracing::info!(level, "log level changed");
                }
            },
            Err(e) => tracing::warn!("ignoring invalid log level {level:?}: {e}"),
        }
    }
}

fn build_filter(level: &str) -> Result<EnvFilter, tracing_subscriber::filter::ParseError> {
    EnvFilter::try_new(format!("warn,tinystream={level}"))
}

#[derive(Clone)]
struct SharedFile(Arc<Mutex<File>>);

impl<'a> MakeWriter<'a> for SharedFile {
    type Writer = SharedFileGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        SharedFileGuard(self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

struct SharedFileGuard<'a>(std::sync::MutexGuard<'a, File>);

impl io::Write for SharedFileGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

pub fn init(paths: &Paths, initial_level: &str) -> anyhow::Result<LogHandle> {
    fs::create_dir_all(&paths.data_dir)?;
    let current = paths.current_log();

    if current.exists() {
        fs::rename(&current, paths.previous_log())?;
    }

    let file = File::create(&current)?;

    let env_override = std::env::var("RUST_LOG").is_ok_and(|v| !v.is_empty());

    let filter = if env_override {
        EnvFilter::from_default_env()
    } else {
        build_filter(initial_level).unwrap_or_else(|_| build_filter("info").unwrap())
    };

    let (filter, reload) = reload::Layer::new(filter);

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(io::stderr)
                .with_ansi(io::stderr().is_terminal())
                .with_target(false),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(SharedFile(Arc::new(Mutex::new(file)))).with_ansi(false))
        .init();

    Ok(LogHandle { reload, env_override })
}
