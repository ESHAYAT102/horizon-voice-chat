#!/bin/sh
# Rebuild upstream, never a fork. Unsupported releases fail closed.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
version=${1:-v0.2.66}
role=${2:-client}
case "$version" in v0.2.66) revision=93e3096e0688cc0e5e3c52e71e666246627d2207;; *) echo "Unsupported Horizon $version; no files changed. A tested patch is required." >&2; exit 1;; esac
case "$role" in client|server) ;; *) echo 'Role must be client or server' >&2; exit 1;; esac
data=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice/native
source=$data/build/$version-$role
mkdir -p "$data/build"
if [ ! -d "$source/.git" ]; then
    test ! -e "$source" || { echo "Refusing existing non-repository: $source" >&2; exit 1; }
    git clone --depth 1 --branch "$version" https://github.com/ludovic111/horizon.git "$source"
fi
test "$(git -C "$source" rev-parse HEAD)" = "$revision" || { echo 'Unexpected upstream revision' >&2; exit 1; }
patch=$root/native/patches/$version.patch
if git -C "$source" diff --quiet && [ -z "$(git -C "$source" ls-files --others --exclude-standard)" ]; then
    git -C "$source" apply --check "$patch"
    git -C "$source" apply "$patch"
elif ! git -C "$source" apply --reverse --check "$patch"; then
    echo 'Source has other changes; refusing to overwrite them.' >&2; exit 1
fi
cd "$source"
if [ "$role" = server ]; then
    cargo build --locked --profile fast -p horizon-client --bin horizon-client --no-default-features --features mods
else
    cargo build --locked --profile fast -p horizon-client --bin horizon-client --features mods
fi
printf '%s\n' "$source/target/fast/horizon-client"
