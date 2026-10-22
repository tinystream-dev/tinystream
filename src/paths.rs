// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::cli::Cli;

#[derive(Debug, Clone, Copy)]
pub enum Source {
    Flag,
    Default,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::Flag => "flag/env",
            Source::Default => "default",
        })
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub config_file: PathBuf,
    pub config_source: Source,
    pub data_dir: PathBuf,
    pub data_source: Source,
}

impl Paths {
    pub fn resolve(cli: &Cli) -> anyhow::Result<Self> {
        let (config_file, config_source) = match &cli.config {
            Some(p) => (absolute(p)?, Source::Flag),
            None => {
                let base = dirs::config_local_dir()
                    .context("can't find a config directory: set $HOME (or $XDG_CONFIG_HOME), or pass --config")?;

                (base.join("tinystream").join("config.toml"), Source::Default)
            },
        };

        let (data_dir, data_source) = match &cli.data_dir {
            Some(p) => (absolute(p)?, Source::Flag),
            None => {
                let base = dirs::data_local_dir()
                    .context("can't find a data directory: set $HOME (or $XDG_DATA_HOME), or pass --data-dir")?;

                (base.join("tinystream"), Source::Default)
            },
        };

        Ok(Self { config_file, config_source, data_dir, data_source })
    }

    pub fn config_dir(&self) -> &Path {
        self.config_file.parent().unwrap_or(Path::new("/"))
    }

    pub fn database(&self) -> PathBuf {
        self.data_dir.join("tinystream.db")
    }

    pub fn current_log(&self) -> PathBuf {
        self.data_dir.join("current.log")
    }

    pub fn previous_log(&self) -> PathBuf {
        self.data_dir.join("previous.log")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }
}

fn absolute(p: &Path) -> anyhow::Result<PathBuf> {
    let p = expand_tilde(p)?;
    if p.is_absolute() { Ok(p) } else { Ok(std::env::current_dir()?.join(p)) }
}

pub fn expand_tilde(p: &Path) -> anyhow::Result<PathBuf> {
    let Ok(rest) = p.strip_prefix("~") else {
        return Ok(p.to_path_buf());
    };

    match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        Some(home) => Ok(PathBuf::from(home).join(rest)),
        None => bail!(
            "`{}` starts with `~`, but $HOME is not set. When running as a service, set HOME in \
             the service definition (e.g. `export HOME=/home/you` in your runit `run` script) or \
             use an absolute path",
            p.display()
        ),
    }
}

pub fn resolve_config_path(raw: &str, config_dir: &Path) -> anyhow::Result<PathBuf> {
    let p = expand_tilde(Path::new(raw))?;
    Ok(if p.is_absolute() { p } else { config_dir.join(p) })
}
