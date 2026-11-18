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
.github/workflows/release.yml  on `v*` tags: release Flatpak bundles -> GitHub release
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
gh pr create --base main --head dev --fill   # GitHub CLI: PRs, CI (gh pr checks), releases
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

- Topic branches (`feat/...`, `fix/...`, `docs/...`, ...) branch off `dev`; each one
  becomes a PR into `dev`.
- `dev`: all new features and fixes land here first, until the maintainer has tested them.
  It only accepts PRs (no direct pushes, no force pushes) and every commit must be signed.
- `main`: receives `dev` once those changes are confirmed stable.
- `release/vX.Y.Z`: when a milestone is reached, `main` is merged into the branch for the
  version currently pending release.
- Never commit straight to `dev`, `main` or a `release/*` branch. Changes reach them
  through PRs (`gh pr create`), merged by the maintainer or on their explicit request.
- Commits are signed with the maintainer's SSH key (`git commit -S`). Agents cannot sign:
  they commit on a topic branch, and the maintainer signs the commits (for example
  `git rebase --exec 'git commit --amend --no-edit -S' dev`) and pushes the branch.

## Releases

A milestone `X.Y.Z` is published by pushing the tag `vX.Y.Z`; `release.yml` then checks
the version, builds the release manifest for x86_64 and aarch64 and creates the GitHub
release with both `.flatpak` bundles and generated notes. The workflow file must exist at
the tagged commit.

1. On a topic branch off `dev` (PR into `dev`), bump the version in `meson.build` and `Cargo.toml` (then `cargo update -p
   mirage --offline` for `Cargo.lock`), add a `<release version="X.Y.Z" date="YYYY-MM-DD">`
   entry at the top of `<releases>` in the metainfo, and run
   `build-aux/update-cargo-sources.sh` if `Cargo.lock` dependencies changed. Commit as
   `chore(release): prepare vX.Y.Z`.
2. PR `dev` → `main`; once CI passes and the maintainer approves, merge.
3. Create `release/vX.Y.Z` from `main` if it does not exist
   (`git push origin main:refs/heads/release/vX.Y.Z`), otherwise PR `main` → it and merge.
4. The maintainer signs the tag on the tip of the release branch and pushes it:
   `git tag -s vX.Y.Z -m "Mirage X.Y.Z" origin/release/vX.Y.Z && git push origin vX.Y.Z`.
   Release tags must be signed; agents never create or push them.
5. Follow it with `gh run watch`; the release appears under `gh release view vX.Y.Z`.
   A failed run: fix on `dev`, bring it through again, delete and re-push the tag
   (`git push --delete origin vX.Y.Z`) only if no release was published.

## Commits

Conventional Commits (`feat(ui): ...`, `fix(adb): ...`, `build(flatpak): ...`, `ci:`,
`docs:`), one feature per commit, imperative subject, body explaining why.
