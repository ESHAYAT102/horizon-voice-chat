#!/usr/bin/env sh
# Remove Horizon Voice Chat from an existing Horizon installation.
# Worlds, options and the stock game are never touched.
set -eu

install_dir=${XDG_BIN_HOME:-$HOME/.local/bin}
data_dir=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice
config_dir=${XDG_CONFIG_HOME:-$HOME/.config}/horizon-voice
game_home=${HORIZON_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/horizon}
unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user

case "${1:-}" in
    -h|--help)
        echo 'usage: uninstall.sh'
        echo
        echo 'Removes the .hmod, the helper, the relay, the user service and the configuration.'
        echo 'Horizon itself, worlds, options and resource packs are left alone.'
        exit 0
        ;;
esac
[ "$#" = 0 ] || { echo "Unknown argument: $*" >&2; exit 1; }

for path in "$install_dir" "$data_dir" "$config_dir" "$game_home"; do
    case "$path" in /*) ;; *) echo "Not an absolute path: $path" >&2; exit 1;; esac
done

# Service first, so it cannot rewrite status while the rest is removed.
if command -v systemctl >/dev/null 2>&1; then
    systemctl --user disable --now horizon-voice-helper 2>/dev/null || true
fi
# Also stop a helper started by hand from this installation; leave any other
# process with the same name alone.
if command -v pgrep >/dev/null 2>&1; then
    for pid in $(pgrep -f 'horizon-voice-helper serve' 2>/dev/null || true); do
        exe=$(readlink "/proc/$pid/exe" 2>/dev/null || true)
        case "$exe" in
            "$data_dir/bin/horizon-voice-helper"|"$install_dir/horizon-voice-helper")
                kill "$pid" 2>/dev/null || true
                ;;
        esac
    done
fi
rm -f -- "$unit_dir/horizon-voice-helper.service"
if command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload 2>/dev/null || true
fi

rm -f -- "$game_home/horizon-mods/horizon_voice.hmod"
rm -rf -- "$game_home/horizon-mods/.horizon-mod-data/horizon_voice"

mods_txt=$game_home/horizon-mods.txt
if [ -f "$mods_txt" ]; then
    # Keep every other line, including other mods' switches.
    tmp=$(mktemp "$mods_txt.tmp.XXXXXX")
    grep -vx 'mod\.horizon_voice:true' "$mods_txt" > "$tmp" || true
    mv -f -- "$tmp" "$mods_txt"
fi

for name in horizon-voice-helper horizon-voice-relay; do
    if [ -f "$install_dir/$name" ]; then
        if grep -q '^# horizon-voice-managed$' "$install_dir/$name" 2>/dev/null; then
            rm -f -- "$install_dir/$name"
            echo "Removed $install_dir/$name"
        else
            echo "Left unrelated $install_dir/$name untouched."
        fi
    fi
done

for file in bin/horizon-voice-helper bin/horizon-voice-relay licenses/VOICE-LICENSE run-relay.sh; do
    rm -f -- "$data_dir/$file"
done
rm -rf -- "$data_dir/examples"
rmdir -- "$data_dir/bin" "$data_dir/licenses" "$data_dir" 2>/dev/null || true

# helper.env and relay.env hold the room key; removing the install should not
# leave either behind.
for name in helper.env relay.env; do
    if [ -f "$config_dir/$name" ]; then
        rm -f -- "$config_dir/$name"
        echo "Removed $config_dir/$name (it held the room key)."
    fi
done
rmdir -- "$config_dir" 2>/dev/null || true

echo 'Uninstalled. Horizon, your worlds, options and resource packs were not changed.'
