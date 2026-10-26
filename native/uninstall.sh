#!/bin/sh
set -eu
[ "$#" = 1 ] || { echo 'Usage: native/uninstall.sh /absolute/game-binary' >&2; exit 2; }
binary=$1
case "$binary" in /*) ;; *) echo 'Use an absolute path.' >&2; exit 2;; esac
data=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice/native
id=$(printf '%s' "$binary" | sha256sum | cut -d' ' -f1)
state=$data/installations/$id
test -f "$state/original" && test "$(cat "$state/binary")" = "$binary" || { echo 'No managed backup found.' >&2; exit 1; }
test ! -L "$binary" || { echo 'Target became a symlink; refusing to replace it.' >&2; exit 1; }
home=$(cat "$state/home")
if [ -f "$binary" ] && [ "$(sha256sum "$binary" | cut -d' ' -f1)" = "$(cat "$state/installed-sha256")" ]; then
    temp=$(mktemp "${binary}.restore.XXXXXX")
    trap 'rm -f "$temp"' EXIT
    cp -p "$state/original" "$temp"
    mv -f "$temp" "$binary"
    echo 'Restored the original game/server binary.'
else
    echo 'Binary changed (possibly an upstream update): left it untouched. Original backup retained.'
fi
if [ -f "$state/created-marker" ]; then rm -f "$home/horizon-voice.enabled"; fi
archive=$state.uninstalled.$(date +%s).$$
mv "$state" "$archive"
echo "Voice disabled. Backup retained in $archive. No launcher, world or other mod files changed."
