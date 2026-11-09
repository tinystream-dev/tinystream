# Desktop music dependency audit

Prepared under #81 for #91. No desktop build has run with these dependencies. Resolution used cargo metadata only. Of 51 registry additions, 41 reuse exact root/mobile versions.

| Crate | Version | Published | Build-time code |
| --- | --- | --- | --- |
| alsa | 0.11.0 | 2025-12-26 | No |
| alsa-sys | 0.4.0 | 2025-12-26 | build.rs |
| coreaudio-rs | 0.14.2 | 2026-04-29 | No |
| cpal | 0.17.3 | 2026-02-18 | build.rs |
| mach2 | 0.5.0 | 2025-08-04 | No |
| num-derive | 0.4.2 | 2024-02-06 | proc macro |
| objc2-audio-toolbox | 0.3.2 | 2025-10-04 | No |
| objc2-avf-audio | 0.3.2 | 2025-10-04 | No |
| objc2-core-audio | 0.3.2 | 2025-10-04 | No |
| objc2-core-audio-types | 0.3.2 | 2025-10-04 | No |

All ten versions are not yanked and older than 30 days. Each publisher matches its previous version. Current owners were checked through crates.io; historical owner-list changes are not exposed there. No advisory entries exist for these ten crates in advisory-db commit 309392b (2026-11-06).

The CPAL build script only reads CPAL_ASIO_DIR and emits configuration. alsa-sys probes ALSA through pkg-config; its optional bindgen feature is disabled. num-derive generates numeric trait implementations; no I/O, network access, subprocesses or obfuscation was found in the reviewed implementation. No other new crate has build-time code. Existing desktop pins remain unchanged. CPAL 0.17.3 is pinned rather than moving to the newer 0.18 line.

Approval pending.
