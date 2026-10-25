#!/usr/bin/env sh
# Maintainer script: build the .hmod and write dist/horizon_voice.hmod plus
# its checksum. Players do not need this: the installer uses the built file.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
fail() { echo "error: $*" >&2; exit 1; }

command -v cargo >/dev/null 2>&1 || fail 'Cargo is not installed.'
command -v cargo-horizon-mod >/dev/null 2>&1 || fail \
    'cargo-horizon-mod is missing. Install it from a Horizon checkout:
  cargo install --path <horizon>/crates/cargo-horizon-mod'

cd "$root/mod"
cargo horizon-mod build

mkdir -p "$root/dist"
cp target/horizon-mod/horizon_voice.hmod "$root/dist/horizon_voice.hmod"
(
    cd "$root/dist"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum horizon_voice.hmod > horizon_voice.hmod.sha256
    else
        shasum -a 256 horizon_voice.hmod > horizon_voice.hmod.sha256
    fi
)
echo "Wrote $root/dist/horizon_voice.hmod"
cat "$root/dist/horizon_voice.hmod.sha256"
