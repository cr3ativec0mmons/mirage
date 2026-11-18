#!/bin/sh
# Vendors the Rust dependencies into the `meson dist` tarball so it builds offline.
export DIST="$1"
export SOURCE_ROOT="$2"

cd "$SOURCE_ROOT"
mkdir -p "$DIST"/.cargo
cargo vendor --locked | sed 's/^directory = ".*"/directory = "vendor"/g' > "$DIST"/.cargo/config.toml
# Move vendor into dist tarball directory
mv vendor "$DIST"
