#!/usr/bin/env sh
set -eu
data=${HORIZON_VOICE_INSTALL:-${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice}
case "${1:-}" in -h|--help) exec "$data/bin/horizon-client" "$@";; esac
config=${XDG_CONFIG_HOME:-$HOME/.config}/horizon-voice/client.env
[ -f "$config" ] || { echo "Configure $config first (see README)." >&2; exit 1; }
. "$config"
: "${HORIZON_VOICE_SERVER:?Configure voice server}"
: "${HORIZON_VOICE_KEY:?Configure shared key}"
: "${HORIZON_VOICE_GAME_SERVERS:?Configure allowed game server addresses}"
resources=${HORIZON_VOICE_RESOURCES:-}
if [ -z "$resources" ]; then
    versions=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-launcher/versions
    resources=$(find "$versions" -mindepth 4 -maxdepth 4 -type d -path '*/.local/horizon-client/26.3' 2>/dev/null | sort -V | tail -1) || resources=
fi
[ -n "$resources" ] && [ -d "$resources" ] || { echo 'Prepare Minecraft 26.3 using the stock Horizon launcher, then set HORIZON_VOICE_RESOURCES.' >&2; exit 1; }
# Separate player data: installing/testing voice must not change stock worlds/options.
export HORIZON_HOME=${HORIZON_VOICE_HOME:-$data/game}
mkdir -p "$HORIZON_HOME"
if [ -n "${HORIZON_VOICE_PLAYER:-}" ]; then
    set -- --name "$HORIZON_VOICE_PLAYER" "$@"
fi
exec "$data/bin/horizon-client" --resources "$resources" "$@"
