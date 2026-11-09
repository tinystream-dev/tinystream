<div align="center">

# tinystream

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logos/logo-inverted.svg">
  <img src="assets/logos/logo.svg" alt="tinystream logo" width="128">
</picture>

A small self-hosted media server for your shows, movies and music.

[![CI](https://github.com/tinystream-dev/tinystream/actions/workflows/ci.yml/badge.svg)](https://github.com/tinystream-dev/tinystream/actions/workflows/ci.yml)
[![Release](https://github.com/tinystream-dev/tinystream/actions/workflows/release.yml/badge.svg)](https://github.com/tinystream-dev/tinystream/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/tinystream-dev/tinystream?sort=semver)](https://github.com/tinystream-dev/tinystream/releases/latest)
[![License: AGPL v3](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust nightly](https://img.shields.io/badge/rust-nightly-orange.svg)](https://www.rust-lang.org)

</div>

## License

Released under the [AGPL-3.0-or-later](./LICENSE) open-source license.

### NVIDIA transcoding

Linux servers with an NVIDIA GPU can use NVENC encoding and NVDEC decoding. Install the proprietary NVIDIA driver (530.41 or newer) and, for Docker, the [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html). Start Docker with:

```sh
docker compose -f docker-compose.yaml -f docker-compose.nvidia.yaml up -d
```

Set `[transcode] hardware = "nvenc"` in the server configuration, or choose NVIDIA (NVENC) in Settings → Server. `cuda-device = 0` selects the first GPU. `hardware = "auto"` tries VA-API, then NVENC, then software. Settings shows the detected encoder or the reason initialization failed. The binary also runs on machines without NVIDIA libraries.

Live conversions, clip exports, and subtitle burn-in use NVENC. HDR sources are tone-mapped to SDR in software before upload. If a stream cannot open a hardware encoder, including when the GPU's concurrent-session limit is reached, that stream falls back to software; other sessions keep using the GPU. Building NVENC support requires clang with its NVPTX backend; CUDA Toolkit and non-free NPP libraries are not used.
