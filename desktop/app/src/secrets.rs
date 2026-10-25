// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::sync::Arc;

use anyhow::Result;
use futures::future::BoxFuture;

use crate::api::Token;

#[derive(Clone)]
pub struct Stored {
    pub token: Option<Token>,
    pub fallback: bool,
}

impl fmt::Debug for Stored {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Stored([redacted])")
    }
}

/// Platform credential storage. Implementations never include secrets in errors.
pub trait Secrets: Send + Sync {
    fn get(&self, id: u64) -> BoxFuture<'_, Result<Stored>>;
    fn set(&self, id: u64, token: Option<Token>) -> BoxFuture<'_, Result<bool>>;
}

pub type Store = Arc<dyn Secrets>;
