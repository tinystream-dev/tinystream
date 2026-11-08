# OS integration (#92)

Linux desktop services sit behind `platform::Desktop`; the shared `integration::Media` state and
commands are independent of D-Bus. The existing video player publishes that state. Music and
Listen Together (#91) can use the same interface when implemented; they are not implemented by
this issue.

The application ID is `dev.tinystream.app`. The old Secret Service attribute is retained so
existing credentials remain accessible. The desktop entry and SVG icon under `app/resources/`
are packaging inputs for #96. The tray also embeds a raster of the existing favicon, so it has
an icon when running from the checkout.

## Services

- `dev.tinystream.app`, `/dev/tinystream/app`: a second process forwards its arguments and exits.
  Opening an empty argument list raises the window. HTTP(S) and `tinystream://` routes preserve
  video timestamps. Saved servers are selected automatically; unknown servers ask to be added,
  and Together links offer a guest join. Invite/password-reset links retain the auth workflow.
- `org.mpris.MediaPlayer2.tinystream`, `/org/mpris/MediaPlayer2`: metadata, playback state,
  microsecond position/length, speed, volume, capabilities and controls. Commands use the same
  player messages as the UI, including room control permissions. Stale SetPosition track IDs are
  ignored. mpv playback restarts emit Seeked, including seeks made by room synchronization.
- `/StatusNotifierItem` and `/Menu`: a StatusNotifierItem and dbusmenu. The menu exposes the
  current title, Play/Pause, Previous, Next, Show and Quit. A successful registration with an
  active tray host enables close-to-tray; losing the watcher restores a hidden window.
- `org.freedesktop.Notifications`: unread, unexpired live inbox arrivals are shown when the window
  is unfocused or hidden. IDs are remembered per server/account in `notifications.json` under
  the state directory. Default-action clicks raise the window and follow the notification link.
  The application does not install a background notification service.

Closing a Together room hides the window only when the tray setting is enabled and a host is
available. Otherwise it exits. Ctrl+Q and Quit always exit. A hidden window is destroyed, retaining only the GPU device for restoration;
its video decoder and room steering are stopped, while room membership remains connected.
Showing the window restores its size and creates the player at the room's current position. Idle inhibition follows
active video, including PiP, and releases when paused, hidden, ended or closed.

Wayland inhibition uses the window's existing display and surface with
`zwp_idle_inhibit_manager_v1`. Unsupported Wayland compositors fall back to the portal's idle
inhibition request; X11 uses ScreenSaver.Inhibit. Guards release the native inhibitor, portal
request or X11 cookie. The guard retains the window and is destroyed before the window.

## Dependency reuse

No new crate versions or build scripts were introduced. Direct dependencies on
`wayland-client 0.31.15` and `wayland-protocols 0.32.13` reuse the versions and features already
locked and built through winit. zbus 5.19.0 was already used by Secret Service and the settings
portal. The lockfile change only adds the two existing packages to the app's dependency list.

## Validation

Run Cargo with one job to avoid saturating the development machine:

```sh
cargo clippy --locked -j 1 --all-targets
cargo test --locked -j 1 --bin tinystream-desktop

dbus-run-session -- cargo test --locked -j 1 --bin tinystream-desktop \
  services_export_controls_and_forward_a_second_launch -- --ignored --nocapture
cargo test --locked -j 1 --bin tinystream-desktop \
  inhibitor_uses_the_window_surface_and_releases_it -- --ignored --nocapture
cargo test --locked -j 1 --bin tinystream-desktop \
  tray_destroys_the_window_and_restores_it_on_the_same_device -- --ignored --nocapture
```

The isolated-bus test checks MPRIS properties and commands, rejection of invalid rates,
second-launch forwarding, the dbusmenu layout/action and notification delivery/activation.
The native test requires a Wayland compositor advertising idle-inhibit. App tests check the
close/hide/quit policy and unknown-server/guest routing. The native tray test confirms the window
is actually destroyed and restoration presents a frame on the same device. Run state-writing tests with a temporary `XDG_STATE_HOME`.

Validated on the development machine (niri/Wayland): 98 regular tests passed, plus the isolated
D-Bus, native idle-inhibit and native window destruction/restoration tests. Clippy and desktop
entry validation passed. These do not replace manual checks of
media widgets, notifications and tray behavior on sway, GNOME and KDE.

Protocol references:
[MPRIS Player](https://specifications.freedesktop.org/mpris/latest/Player_Interface.html),
[portal Inhibit](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Inhibit.html),
[Wayland idle-inhibit](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/unstable/idle-inhibit/idle-inhibit-unstable-v1.xml).
