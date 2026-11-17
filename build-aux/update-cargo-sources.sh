#!/bin/sh
# Regenerates build-aux/cargo-sources.json from Cargo.lock. The release
# manifest builds offline and needs it; run this after changing dependencies.
set -eu

cd "$(dirname "$0")/.."
generator_url=https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

curl -fsSL -o "$tmp/flatpak-cargo-generator.py" "$generator_url"
if command -v uv >/dev/null 2>&1; then
    # The generator declares its dependencies inline
    uv run --script "$tmp/flatpak-cargo-generator.py" Cargo.lock -o build-aux/cargo-sources.json
else
    # Requires the aiohttp, PyYAML and tomlkit Python modules
    python3 "$tmp/flatpak-cargo-generator.py" Cargo.lock -o build-aux/cargo-sources.json
fi
