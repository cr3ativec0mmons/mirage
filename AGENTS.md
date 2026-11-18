# AGENTS.md

Mirage (`tech.vitainside.mirage`) is a GNOME app in Rust (GTK 4 + libadwaita) that lists
Android devices from the ADB server and mirrors them by launching **scrcpy in its own
window** (Mirage never embeds the video). Flatpak is the primary target.

## Stack

- Rust edition 2024, MSRV 1.92. `gtk4` 0.11.5 (feature `gnome_49`), `libadwaita` 0.9.2
  (`v1_8`); glib/gio come from the `gtk::` re-exports. Also `mdns-sd`, `qrcode`,
  `gettext-rs`, `getrandom`, `async-channel`, `tracing`.
- Meson drives Cargo (GNOME Rust template). Runtime: GNOME 51 (GTK 4.24, libadwaita 1.10).
- UI is GtkBuilder `.ui` XML (not Blueprint), compiled by `build.rs` into a gresource
  embedded in the binary.
- Settings: GSettings (`data/tech.vitainside.mirage.gschema.xml.in`). Inside Flatpak
  they land in `$XDG_CONFIG_HOME`.

## Layout

```
src/main.rs              gettext setup, loads resources, runs the app
build.rs                 embeds the gresource; compiles the Devel schema for plain cargo
src/config.rs            APP_ID/PROFILE/VERSION/paths from MIRAGE_* env (set by Meson)
src/application.rs       AdwApplication: app.quit/preferences/about, accels
src/device.rs            Device GObject (state, wireless, activity)
src/recordings.rs        ~/Videos/Mirage paths
src/i18n.rs              gettext_f / ngettext_f with {named} placeholders
src/adb/client.rs        blocking ADB server protocol client (pair/connect/shell/track)
src/adb/devices.rs       `devices -l` parser
src/adb/tracker.rs       thread following host:track-devices-l -> async_channel
src/adb/discovery.rs     mDNS: _adb-tls-pairing / _adb-tls-connect
src/scrcpy/options.rs    settings -> scrcpy args (unit tested)
src/scrcpy/session.rs    gio::Subprocess, SIGTERM to stop, exit 2 = disconnected
src/widgets/             window, device_row, pairing_dialog, qr_code, preferences_dialog
data/resources/          gresource xml, ui/*.ui, shortcuts-dialog.ui, style.css, icons/
data/                    desktop, metainfo, gschema, D-Bus service (*.in templates)
data/icons/hicolor/      app icons (regular, .Devel, symbolic)
po/                      POTFILES.in, LINGUAS, mirage.pot, es.po
build-aux/               Flatpak manifests (+ modules/), dist-vendor.sh, update-cargo-sources.sh
.github/workflows/ci.yml Fedora lint/test + Flatpak x86_64/aarch64
```

## Commands

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test   # no Meson needed
meson setup _build -Dprofile=development && meson compile -C _build
meson test -C _build                 # desktop/metainfo/gschema validation + cargo test
meson devenv -C _build ./src/mirage  # run from the build tree
cargo run                            # also works: Devel profile, no translations
meson compile -C _build mirage-pot && meson compile -C _build mirage-update-po
flatpak-builder --user --install --force-clean _flatpak build-aux/tech.vitainside.mirage.Devel.json
```

## Conventions and gotchas

- Every user-visible string is translatable. Use `gettext`, `ngettext`, `pgettext`
  (context when gender/meaning differs), `gettext_f`, `ngettext_f`. Add new files with
  strings to `po/POTFILES.in`; update `po/es.po` too.
- **xgettext skips strings inside path-qualified macros**: write `clone!(...)` /
  `closure_local!(...)` (imported), never `glib::clone!(...)`.
- Codec/format names (H.264, AV1, Opus, MP4, VA-API) are not translated.
- Parameterless window actions go in `setup_actions` (`gio::ActionEntry`, exported on
  D-Bus); `klass.install_action` is only for actions with a serial parameter.
- Blocking ADB calls run in `gio::spawn_blocking`; never block the main loop.
- Resource base path is always `/tech/vitainside/mirage`, independent of profile.
- Recordings folder name `Mirage` must match the Flatpak permission `xdg-videos/Mirage`.
- Follow GNOME HIG: adaptive down to 360x294, header capitalization for titles,
  sentence case for descriptions, icon-only buttons need tooltips + accessible labels.
- Comments are sparse; match the existing style. No `unsafe` except the documented
  `setlocale` call.

## Flatpak

- Modules: abseil-cpp, protobuf, googletest (build-only, `cleanup: *`), android-tools
  (only `adb`, mDNS off), scrcpy (built from source, prebuilt `scrcpy-server`; do NOT set
  `compile_server=false`, it skips installing the server).
- Devel manifest builds with network; release manifest is offline and needs
  `build-aux/cargo-sources.json` (run `build-aux/update-cargo-sources.sh`).
- Keep `x-checker-data` and sha256 in sync when bumping module versions.

## Branches

- `dev`: all new features and fixes land here first, until the maintainer has tested them.
- `main`: receives `dev` once those changes are confirmed stable.
- `release/vX.Y.Z`: when a milestone is reached, `main` is merged into the branch for the
  version currently pending release.
- Never commit straight to `main` or a `release/*` branch. Merges into them are done by
  the maintainer.

## Commits

Conventional Commits (`feat(ui): ...`, `fix(adb): ...`, `build(flatpak): ...`, `ci:`,
`docs:`), one feature per commit, imperative subject, body explaining why.
