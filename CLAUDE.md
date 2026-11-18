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
- Do not commit or push unless asked. Work on `dev` (see Branches in AGENTS.md): new
  fixes and features stay there until the user has tested them.
- `gh` (GitHub CLI) is available. Use it to open PRs (`dev` → `main`, `main` →
  `release/vX.Y.Z`), check CI (`gh pr checks`, `gh run watch`) and releases. Opening a
  PR is fine once the user asks for it; merging it (`gh pr merge --merge`) or pushing a
  release tag only when the user explicitly says so for that PR or version. Never push
  directly to `main` or `release/*`. If `gh auth status` fails, ask the user to run
  `! gh auth login`.
- For a milestone, follow Releases in AGENTS.md step by step; the version bump goes
  through `dev` like any other change.
