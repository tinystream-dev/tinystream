// SPDX-License-Identifier: AGPL-3.0-or-later

uniffi::setup_scaffolding!();

/// The workspace version this was built as.
#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
