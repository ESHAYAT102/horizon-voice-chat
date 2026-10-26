#!/bin/sh
# Explicit targets only: never edits a launcher, world or server configuration.
set -eu
if [ "$#" -lt 2 ] || [ "$#" -gt 4 ]; then
    echo 'Usage: native/install.sh /absolute/game-binary /absolute/game-home [v0.2.66] [client|server]' >&2; exit 2
fi
binary=$1; home=$2; version=${3:-v0.2.66}; role=${4:-client}
case "$binary:$home" in /*:/*) ;; *) echo 'Use absolute paths.' >&2; exit 2;; esac
test -f "$binary" && test ! -L "$binary" && test -w "$binary" && test -d "$home" || { echo 'Target must be an existing writable regular binary (not a symlink) and game folder.' >&2; exit 1; }
command -v sha256sum >/dev/null
data=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice/native
id=$(printf '%s' "$binary" | sha256sum | cut -d' ' -f1)
state=$data/installations/$id
test ! -e "$state" || { echo 'Already installed or a backup exists. Uninstall first.' >&2; exit 1; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
sh "$root/native/build.sh" "$version" "$role"
built=$data/build/$version-$role/target/fast/horizon-client
test -x "$built"
umask 077
mkdir -p "$state"
cp -p "$binary" "$state/original"
printf '%s\n' "$binary" > "$state/binary"
printf '%s\n' "$home" > "$state/home"
printf '%s\n' "$version" > "$state/version"
sha256sum "$built" | cut -d' ' -f1 > "$state/installed-sha256"
if [ ! -e "$home/horizon-voice.enabled" ]; then
    touch "$state/created-marker"
    touch "$home/horizon-voice.enabled"
fi
temp=$(mktemp "${binary}.voice.XXXXXX")
trap 'rm -f "$temp"' EXIT
cp "$built" "$temp"
chmod 755 "$temp"
mv -f "$temp" "$binary"
echo 'Installed native voice. Restart the game/server. Starts muted; V toggles the microphone.'
echo 'The launcher was not modified. Upstream updates can replace this binary; reapply only a tested patch.'
