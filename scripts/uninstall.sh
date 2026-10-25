#!/usr/bin/env sh
set -eu
install_dir=${XDG_BIN_HOME:-$HOME/.local/bin}
data_dir=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice
config_dir=${XDG_CONFIG_HOME:-$HOME/.config}/horizon-voice
purge=0
case "${1:-}" in
    --purge) purge=1; shift;;
    -h|--help) echo 'usage: uninstall.sh [--purge]'; echo 'By default preserves voice config and separate game saves. --purge removes voice config only.'; exit 0;;
esac
[ "$#" = 0 ] || { echo "Unknown argument: $*" >&2; exit 1; }
for path in "$install_dir" "$data_dir" "$config_dir"; do
    case "$path" in /*) ;; *) echo 'Paths must be absolute.' >&2; exit 1;; esac
done
# Only remove our own integration files. Never remove stock Horizon or its worlds.
for name in horizon-voice horizon-voice-relay; do
    if [ -f "$install_dir/$name" ]; then
        if grep -q '^# horizon-voice-managed$' "$install_dir/$name"; then
            rm -f -- "$install_dir/$name"
            echo "Removed $install_dir/$name"
        else echo "Left unrelated $install_dir/$name untouched."; fi
    fi
done
for file in bin/horizon-client bin/horizon-voice-relay run-client.sh run-relay.sh examples/client.env.example examples/relay.env.example licenses/VOICE-LICENSE licenses/HORIZON-LICENSE licenses/HORIZON-THIRD-PARTY-NOTICES.md; do
    rm -f -- "$data_dir/$file"
done
if [ "$purge" = 1 ]; then
    rm -f -- "$config_dir/client.env" "$config_dir/relay.env"
    rmdir -- "$config_dir" 2>/dev/null || true
fi
echo "Uninstalled. Separate game saves remain in $data_dir/game."
echo 'Stock Horizon, server worlds, build prerequisites, services and firewall rules were not changed.'
echo 'Stop any manually installed voice service before removing its binaries.'
