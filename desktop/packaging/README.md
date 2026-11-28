# tinystream for Linux

The desktop app for [tinystream](https://github.com/tinystream-dev/tinystream), a small self-hosted
media server for your shows, movies, music and books. It needs a tinystream server to connect to.

## Installing

From this directory:

```sh
./install.sh
```

That installs the app for you alone, into `~/.local`: it shows up in your applications menu, and
`tinystream://` links (Watch Together invites, for one) open in it. To install it for everyone:

```sh
sudo ./install.sh --prefix /usr/local
```

`./install.sh --uninstall` (with the same `--prefix`) removes it again. Your servers and settings
stay in `~/.config/tinystream` and `~/.local/state/tinystream`; sign-ins are in your keyring.

You can also run `./tinystream-desktop` from here without installing it. The AppImage from the same
release needs no installing either: make it executable and open it.

## Requirements

- x86_64 Linux with glibc 2.39 or newer: Ubuntu 24.04, Fedora 40, or any rolling distribution.
- A Vulkan driver: Mesa's for AMD and Intel, or NVIDIA's own.
- Wayland or X11.
- PipeWire's and ALSA's libraries (`libpipewire-0.3.so.0` and `libasound.so.2`), even if sound
  goes through PulseAudio: desktop installs have them, but a minimal system may need its
  `pipewire` and `alsa-lib` packages.
- For hardware decoding, VA-API on AMD and Intel (Mesa's `va` drivers, or `intel-media-driver`),
  and CUDA (part of NVIDIA's driver) on NVIDIA.

## Licenses

tinystream is free software under the GNU Affero General Public License version 3 or later; see
`LICENSE`. `THIRD_PARTY_LICENSES` has the notices of everything it's built with, and where to get
their source.
