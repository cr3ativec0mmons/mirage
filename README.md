# Mirage

Mirror and control your Android devices from GNOME.

Mirage lists the Android devices known to the ADB server, connected with a
USB cable or wirelessly, and mirrors them with [scrcpy](https://github.com/Genymobile/scrcpy).
Each device gets its own scrcpy window, titled with the name of the device.

- Live device list, updated as devices are plugged in, authorized or connected
- Wireless pairing with a QR code or a pairing code, discovered over mDNS
- Automatic reconnection of paired devices on the network
- Screen and audio recording to `~/Videos/Mirage`
- Video codec, resolution, frame rate, bit rate and hardware decoding settings
- Virtual displays, keyboard, mouse and gamepad forwarding
- Adaptive interface, from phones running Phosh to large displays

## Building

### GNOME Builder

Open the project folder in Builder. It picks up
`build-aux/tech.vitainside.mirage.Devel.json` and builds the development
version with the GNOME 51 SDK.

### Flatpak

```sh
flatpak install --user flathub org.gnome.Sdk//51 org.freedesktop.Sdk.Extension.rust-stable//26.08
flatpak-builder --user --install --force-clean _flatpak build-aux/tech.vitainside.mirage.Devel.json
flatpak run tech.vitainside.mirage.Devel
```

The development manifest downloads the Rust crates during the build. The
release manifest, `build-aux/tech.vitainside.mirage.json`, builds offline and
needs `build-aux/cargo-sources.json`, which `build-aux/update-cargo-sources.sh`
generates from `Cargo.lock`.

Both manifests build adb (from [android-tools](https://github.com/nmeum/android-tools))
and scrcpy from source against the GNOME runtime, so they work on x86_64 and
aarch64, and scrcpy uses the VA-API drivers of the runtime. Abseil, Protobuf
and GoogleTest are only needed to build adb and are removed from the final app.

### Native

Requires Rust 1.92 or later, Meson, GTK 4.20 and libadwaita 1.8, plus `adb`
and `scrcpy` at runtime.

```sh
meson setup _build -Dprofile=development
meson compile -C _build
meson devenv -C _build ./src/mirage
```

`cargo check`, `cargo clippy` and `cargo test` also work on their own, without
Meson.

## Sandbox permissions

| Permission | Reason |
| --- | --- |
| `--share=network` | Talk to the ADB server, connect to and discover devices over Wi-Fi |
| `--device=usb` | Access devices connected with a USB cable |
| `--device=input` | Forward gamepads |
| `--device=dri` | Render and decode the video with the GPU |
| `--socket=pulseaudio` | Play the audio of the device |
| `--filesystem=~/.android:create` | Share the ADB keys with the host, so devices authorized there also work in Mirage |
| `--filesystem=xdg-videos/Mirage:create` | Save recordings |

If an ADB server is already running on the host, Mirage uses it. Otherwise the
bundled adb starts one inside the sandbox.

USB devices show up only if the user can open them. Most distributions grant
access through the `android-udev-rules` package or systemd's `uaccess` rules.

## Translations

Mirage uses gettext. After changing translatable strings:

```sh
meson compile -C _build mirage-pot        # Update po/mirage.pot
meson compile -C _build mirage-update-po  # Merge it into the po/*.po files
```

To add a language, add its code to `po/LINGUAS` and create the catalog with
`msginit -i po/mirage.pot -o po/<code>.po -l <code>`.

Strings in Rust use `gettext`, `ngettext` and `pgettext`, or `gettext_f` and
`ngettext_f` for named `{placeholders}`. xgettext skips strings inside
path-qualified macros, so import `clone!` and `closure_local!` instead of
writing `glib::clone!`.

## Icons

The application icons go in `data/icons`, see
[data/icons/README.md](data/icons/README.md).

## License

GPL-3.0-or-later. scrcpy and adb are licensed under the Apache License 2.0.
