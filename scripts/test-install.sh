#!/usr/bin/env sh
# Isolated integration test: no real player data, config, services or firewall changes.
set -eu
source_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/horizon-voice-test.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
export XDG_BIN_HOME="$work/bin" XDG_DATA_HOME="$work/data" XDG_CONFIG_HOME="$work/config"
export HORIZON_VOICE_SOURCE="$source_dir"
sh "$source_dir/scripts/install.sh" --relay-only
test -x "$XDG_BIN_HOME/horizon-voice-relay"
"$XDG_BIN_HOME/horizon-voice-relay" --help >/dev/null
"$XDG_BIN_HOME/horizon-voice-relay" --version >/dev/null
mkdir -p "$XDG_CONFIG_HOME/horizon-voice" "$XDG_DATA_HOME/horizon-voice/game/saves"
echo sentinel > "$XDG_CONFIG_HOME/horizon-voice/client.env"
echo sentinel > "$XDG_DATA_HOME/horizon-voice/game/saves/KEEP"
sh "$source_dir/scripts/uninstall.sh"
test ! -e "$XDG_BIN_HOME/horizon-voice-relay"
test -f "$XDG_CONFIG_HOME/horizon-voice/client.env"
test -f "$XDG_DATA_HOME/horizon-voice/game/saves/KEEP"
sh "$source_dir/scripts/uninstall.sh" --purge
test ! -e "$XDG_CONFIG_HOME/horizon-voice/client.env"
test -f "$XDG_DATA_HOME/horizon-voice/game/saves/KEEP"
sh "$source_dir/scripts/uninstall.sh" --purge
# Do not clobber somebody else's command.
printf '#!/bin/sh\necho unrelated\n' > "$XDG_BIN_HOME/horizon-voice"
if sh "$source_dir/scripts/install.sh" --relay-only > "$work/error.log" 2>&1; then
    echo 'FAIL: installer overwrote unrelated command' >&2; exit 1
fi
grep -q unrelated "$XDG_BIN_HOME/horizon-voice"
echo 'PASS: install, uninstall, repeated uninstall, config/save preservation, ownership guard'
