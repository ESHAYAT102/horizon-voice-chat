#!/usr/bin/env sh
# Test launcher quoting/config/resource discovery without starting the game or mic.
set -eu
source_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/horizon-voice-wrapper.XXXXXX")
trap 'rm -rf -- "$work"' EXIT
export HORIZON_VOICE_INSTALL="$work/app" XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data"
mkdir -p "$work/app/bin" "$work/config/horizon-voice" "$work/data/horizon-launcher/versions/v0.2.48/.local/horizon-client/26.3"
cat > "$work/app/bin/horizon-client" <<'EOF'
#!/bin/sh
printf '%s\n' "$@"
EOF
chmod +x "$work/app/bin/horizon-client"
cat > "$work/config/horizon-voice/client.env" <<'EOF'
export HORIZON_VOICE_KEY=0707070707070707070707070707070707070707070707070707070707070707
export HORIZON_VOICE_SERVER=127.0.0.1:24454
export HORIZON_VOICE_GAME_SERVERS=127.0.0.1:25566
export HORIZON_VOICE_PLAYER='name with spaces'
EOF
sh "$source_dir/scripts/run-client.sh" --frames 10 > "$work/actual"
printf '%s\n' --resources "$work/data/horizon-launcher/versions/v0.2.48/.local/horizon-client/26.3" --name 'name with spaces' --frames 10 > "$work/expected"
cmp "$work/expected" "$work/actual"
test -d "$work/app/game"
rm -f "$work/config/horizon-voice/client.env"
# Help should work even before first-time configuration.
sh "$source_dir/scripts/run-client.sh" --help | grep -q -- --help
echo 'PASS: resource discovery, argument quoting, separate game directory, help without configuration'
