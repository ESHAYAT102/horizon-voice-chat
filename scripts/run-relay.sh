#!/usr/bin/env sh
set -eu
data=${HORIZON_VOICE_INSTALL:-${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice}
case "${1:-}" in -h|--help|-V|--version) exec "$data/bin/horizon-voice-relay" "$@";; esac
config=${XDG_CONFIG_HOME:-$HOME/.config}/horizon-voice/relay.env
[ -f "$config" ] || { echo "Configure $config first (see README)." >&2; exit 1; }
. "$config"
exec "$data/bin/horizon-voice-relay" "$@"
