#!/bin/sh
# Explicit targets only: never edits a launcher, world or server configuration.
set -eu
if [ "$#" -lt 2 ] || [ "$#" -gt 4 ]; then
    echo 'Usage: native/install.sh /absolute/game-binary /absolute/game-home [v0.2.66] [client|server]' >&2; exit 2
fi
binary=$1; home=$2; version=${3:-v0.2.66}; role=${4:-client}
case "$version" in v0.2.66) ;; *) echo "Unsupported Horizon $version; no files changed." >&2; exit 1;; esac
case "$role" in client|server) ;; *) echo 'Role must be client or server.' >&2; exit 1;; esac
case "$binary:$home" in /*:/*) ;; *) echo 'Use absolute paths.' >&2; exit 2;; esac
test -f "$binary" && test ! -L "$binary" && test -w "$binary" && test -d "$home" || { echo 'Target must be an existing writable regular binary (not a symlink) and game folder.' >&2; exit 1; }
command -v sha256sum >/dev/null
data=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice/native
id=$(printf '%s' "$binary" | sha256sum | cut -d' ' -f1)
state=$data/installations/$id
test ! -e "$state" || { echo 'Already installed or a backup exists. Uninstall first.' >&2; exit 1; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if [ -n "${HORIZON_VOICE_PREBUILT:-}" ]; then
    # Explicit admin-supplied artifact. Never download or implicitly trust one.
    built=$HORIZON_VOICE_PREBUILT
    test "${HORIZON_VOICE_PREBUILT_VERSION:-}" = "$version" || { echo 'Prebuilt release does not match target.' >&2; exit 1; }
    test -n "${HORIZON_VOICE_PREBUILT_SHA256:-}" || { echo 'A trusted artifact SHA-256 is required.' >&2; exit 1; }
    test "$(sha256sum "$built" | cut -d' ' -f1)" = "$HORIZON_VOICE_PREBUILT_SHA256" || { echo 'Artifact checksum mismatch.' >&2; exit 1; }
else
    sh "$root/native/build.sh" "$version" "$role"
    built=$data/build/$version-$role/target/fast/horizon-client
fi
test -x "$built"
if command -v ldd >/dev/null 2>&1 && [ "$(od -An -tx1 -N4 "$built" | tr -d ' \n')" = 7f454c46 ]; then
    # Check on the destination host before replacing anything (glibc can differ).
    libraries=$(ldd "$built" 2>&1) || true
    case "$libraries" in *'not found'*) printf 'Artifact cannot run on this machine:\n%s\n' "$libraries" >&2; exit 1;; esac
fi
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
