#!/usr/bin/env sh
# Isolated integration test: no real player data, config, service or firewall
# changes. Builds the helper and installs it into throwaway XDG folders.
set -eu
source_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/horizon-voice-test.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
export XDG_BIN_HOME="$work/bin" XDG_DATA_HOME="$work/data" XDG_CONFIG_HOME="$work/config"
export HORIZON_HOME="$work/game"
export HORIZON_VOICE_SOURCE="$source_dir"

mkdir -p "$HORIZON_HOME/horizon-mods" "$HORIZON_HOME/saves/Keep"
printf 'mod.example_mod:true\n' > "$HORIZON_HOME/horizon-mods.txt"
echo sentinel > "$HORIZON_HOME/saves/Keep/level.dat"

# The mod alone: no helper, no build tools beyond git.
sh "$source_dir/scripts/install.sh" --mod-only
test -f "$HORIZON_HOME/horizon-mods/horizon_voice.hmod"
grep -qx 'mod.horizon_voice:true' "$HORIZON_HOME/horizon-mods.txt"
grep -qx 'mod.example_mod:true' "$HORIZON_HOME/horizon-mods.txt"
test ! -e "$XDG_BIN_HOME/horizon-voice-helper"

# The full install: mod, helper, wrapper and private config.
sh "$source_dir/scripts/install.sh" --no-service
test -x "$XDG_BIN_HOME/horizon-voice-helper"
test -x "$XDG_DATA_HOME/horizon-voice/bin/horizon-voice-helper"
test -f "$XDG_DATA_HOME/horizon-voice/licenses/VOICE-LICENSE"
test -f "$XDG_CONFIG_HOME/horizon-voice/helper.env"
mode=$(stat -c %a "$XDG_CONFIG_HOME/horizon-voice/helper.env" 2>/dev/null || stat -f %Lp "$XDG_CONFIG_HOME/horizon-voice/helper.env")
[ "$mode" = 600 ] || { echo "FAIL: helper.env mode is $mode, expected 600" >&2; exit 1; }
HORIZON_VOICE_HOME="$HORIZON_HOME" "$XDG_BIN_HOME/horizon-voice-helper" status | grep -q 'no status yet'

# Running it again replaces its own files and keeps one switch line.
sh "$source_dir/scripts/install.sh" --no-service
[ "$(grep -c '^mod\.horizon_voice:' "$HORIZON_HOME/horizon-mods.txt")" = 1 ]

# Uninstall: voice files go, everything else stays.
sh "$source_dir/scripts/uninstall.sh"
test ! -e "$HORIZON_HOME/horizon-mods/horizon_voice.hmod"
test ! -e "$HORIZON_HOME/horizon-mods/.horizon-mod-data/horizon_voice"
test ! -e "$XDG_BIN_HOME/horizon-voice-helper"
test ! -e "$XDG_DATA_HOME/horizon-voice"
test ! -e "$XDG_CONFIG_HOME/horizon-voice/helper.env"
grep -qx 'mod.example_mod:true' "$HORIZON_HOME/horizon-mods.txt"
test -f "$HORIZON_HOME/saves/Keep/level.dat"
sh "$source_dir/scripts/uninstall.sh"  # a second time must not fail

# Do not take over a command somebody else installed.
mkdir -p "$XDG_BIN_HOME"
printf '#!/bin/sh\necho unrelated\n' > "$XDG_BIN_HOME/horizon-voice-helper"
if sh "$source_dir/scripts/install.sh" --no-service > "$work/error.log" 2>&1; then
    echo 'FAIL: installer overwrote an unrelated command' >&2
    exit 1
fi
grep -q unrelated "$XDG_BIN_HOME/horizon-voice-helper"
sh "$source_dir/scripts/uninstall.sh" >/dev/null
grep -q unrelated "$XDG_BIN_HOME/horizon-voice-helper"

echo 'PASS: mod-only, full install, reinstall, uninstall, preservation, ownership guard'
