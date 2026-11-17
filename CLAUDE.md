@AGENTS.md

## Claude Code notes

- Before finishing a change, run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`
  and `meson test -C _build`. Regenerate translations when strings change.
- Reference sources are cloned in `~/Projects`: `scrcpy`, `video-downloader` (UI/design
  reference), `ffmpeg`. `~/Projects/Espejismo` is an older, abandoned attempt.
- Visual checks run headless: a private dbus-broker plus `gnome-shell --headless` with
  `ADW_DISABLE_PORTAL=1 GDK_DEBUG=no-portals`, screenshots via `org.gnome.Shell.Screenshot`
  (see the `headless-gui-screenshots` memory). Do not open windows on the user's desktop.
- In this sandbox, `flatpak-builder` needs `--disable-rofiles-fuse` when the state dir is
  in the scratchpad.
- Do not commit or push unless asked; branch off `main` first.
