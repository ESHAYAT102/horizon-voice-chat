#!/usr/bin/env sh
# Install Horizon Voice Chat on top of an existing Horizon installation:
# a .hmod for the game, and a small per-user helper for the microphone,
# playback and the relay connection. Nothing here replaces Horizon.
set -eu

repo_url=${HORIZON_VOICE_REPO:-https://github.com/ESHAYAT102/horizon-voice-chat.git}
install_dir=${XDG_BIN_HOME:-$HOME/.local/bin}
data_dir=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice
config_dir=${XDG_CONFIG_HOME:-$HOME/.config}/horizon-voice
game_home=${HORIZON_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/horizon}
source_dir=${HORIZON_VOICE_SOURCE:-}
mode=all
service=yes
work=
publish_temp=
mods_temp=

has() { command -v "$1" >/dev/null 2>&1; }
fail() { echo "error: $*" >&2; exit 1; }
usage() {
    echo 'usage: install.sh [--mod-only | --relay-only] [--no-service]'
    echo
    echo '  (default)      install the .hmod, the helper and a user service'
    echo '  --mod-only     install only the .hmod (the game shows the HUD, no audio)'
    echo '  --relay-only   install only the relay (the machine hosting voice)'
    echo '  --no-service   build the helper but do not enable the user service'
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --mod-only) mode=mod;;
        --relay-only) mode=relay;;
        --no-service) service=no;;
        -h|--help) usage; exit 0;;
        *) usage >&2; fail "unknown option: $1";;
    esac
    shift
done

case $(uname -s) in Linux) ;; *) fail 'This first release supports Linux only.';; esac
for path in "$install_dir" "$data_dir" "$config_dir" "$game_home"; do
    case "$path" in /*) ;; *) fail "Not an absolute path: $path";; esac
    case "$path" in *"'"*|*'
'*) fail 'Install paths must not contain quotes or newlines.';; esac
done
[ "$mode" = relay ] || [ -d "$game_home" ] \
    || fail "No Horizon game folder at $game_home. Start Horizon once, or set HORIZON_HOME."

cleanup() {
    [ -z "$publish_temp" ] || rm -f -- "$publish_temp"
    [ -z "${mods_temp:-}" ] || rm -f -- "$mods_temp"
    [ -z "$work" ] || rm -rf -- "$work"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

confirm() {
    [ -r /dev/tty ] && [ -w /dev/tty ] || fail 'A terminal is required to confirm dependency installation.'
    printf '%s [y/N] ' "$1" >/dev/tty
    IFS= read -r answer </dev/tty || answer=
    case "$answer" in y|Y|yes|YES) return 0;; *) return 1;; esac
}
root() { if [ "$(id -u)" -eq 0 ]; then "$@"; else sudo "$@"; fi; }

# --- build tools -----------------------------------------------------------
missing=0
if [ "$mode" = mod ]; then
    has git || missing=1
else
    for tool in git cargo rustc cc pkg-config; do has "$tool" || missing=1; done
    [ "$mode" = relay ] || pkg-config --exists alsa libudev 2>/dev/null || missing=1
fi
if [ "$missing" = 1 ]; then
    confirm 'Install the required build tools and development headers?' \
        || fail 'Install Rust 1.90+, git, a C compiler, pkg-config and ALSA/udev headers, then retry.'
    if has pacman; then
        # Never use pacman -Sy: partial upgrades are unsafe on Arch.
        if [ "$mode" = mod ]; then root pacman -S --needed rust git base-devel;
        elif [ "$mode" = relay ]; then root pacman -S --needed rust git base-devel;
        else root pacman -S --needed rust git base-devel alsa-lib systemd-libs; fi
    elif has apt-get; then
        root apt-get update
        if [ "$mode" = mod ]; then root apt-get install -y cargo rustc git build-essential pkg-config;
        elif [ "$mode" = relay ]; then root apt-get install -y cargo rustc git build-essential pkg-config;
        else root apt-get install -y cargo rustc git build-essential pkg-config libasound2-dev libudev-dev; fi
    elif has dnf; then
        if [ "$mode" = mod ]; then root dnf install -y cargo rust git gcc pkgconf-pkg-config;
        elif [ "$mode" = relay ]; then root dnf install -y cargo rust git gcc pkgconf-pkg-config;
        else root dnf install -y cargo rust git gcc pkgconf-pkg-config alsa-lib-devel systemd-devel; fi
    else fail 'Supported dependency installers: pacman, apt-get, dnf. Install prerequisites manually.'; fi
fi
cargo --version >/dev/null 2>&1 || { [ "$mode" = mod ] || fail 'Cargo is unavailable; configure your Rust toolchain first.'; }
rustc --version >/dev/null 2>&1 || { [ "$mode" = mod ] || fail 'Rust is unavailable; configure your Rust toolchain first.'; }
if [ "$mode" != mod ]; then
    rust_version=$(rustc --version | awk '{print $2}')
    printf '%s\n' "$rust_version" | awk -F. '{ exit !($1 > 1 || ($1 == 1 && $2 >= 90)) }' \
        || fail "Rust 1.90 or newer is required (found $rust_version). Update your toolchain and retry."
fi

# --- sources ---------------------------------------------------------------
work=$(mktemp -d "${TMPDIR:-/tmp}/horizon-voice.XXXXXX")
if [ -z "$source_dir" ]; then
    echo "Cloning $repo_url"
    git clone --depth 1 "$repo_url" "$work/source"
    source_dir=$work/source
fi
source_dir=$(CDPATH= cd -- "$source_dir" && pwd)

publish_file() {
    # Rename rather than overwrite, so reinstalling a running binary is safe.
    publish_temp=$(mktemp "$2.tmp.XXXXXX")
    cp "$1" "$publish_temp"
    chmod "$3" "$publish_temp"
    mv -f -- "$publish_temp" "$2"
    publish_temp=
}
publish_command() {
    # Never take over a command that this project did not install.
    if [ -e "$2" ] && ! grep -q '^# horizon-voice-managed$' "$2" 2>/dev/null; then
        fail "$2 exists and was not installed by Horizon Voice Chat. Remove it or set XDG_BIN_HOME elsewhere."
    fi
    publish_file "$1" "$2" 755
}

# --- relay (host only) -----------------------------------------------------
if [ "$mode" = relay ]; then
    echo 'Building the voice relay (no game or microphone dependencies).'
    (cd "$source_dir" && cargo build --locked --release --bin horizon-voice-relay)
    mkdir -p "$data_dir/bin"
    publish_file "$source_dir/target/release/horizon-voice-relay" "$data_dir/bin/horizon-voice-relay" 755
    cp "$source_dir/scripts/run-relay.sh" "$data_dir/run-relay.sh"
    mkdir -p "$install_dir"
    {
        echo '#!/usr/bin/env sh'
        echo '# horizon-voice-managed'
        echo 'set -eu'
        printf "export HORIZON_VOICE_INSTALL='%s'\n" "$data_dir"
        printf "exec sh '%s/run-relay.sh' \"\$@\"\n" "$data_dir"
    } > "$work/wrapper"
    chmod 755 "$work/wrapper"
    publish_command "$work/wrapper" "$install_dir/horizon-voice-relay"
    mkdir -p "$data_dir/licenses" "$data_dir/examples"
    cp "$source_dir/LICENSE" "$data_dir/licenses/VOICE-LICENSE"
    cp "$source_dir/config/"*.example "$data_dir/examples/"
    echo "Relay installed in $data_dir. Configure it with $data_dir/examples/relay.env.example."
    echo 'No service was enabled and no firewall ports were opened automatically.'
    exit 0
fi

# --- the mod ---------------------------------------------------------------
hmod=$source_dir/dist/horizon_voice.hmod
[ -f "$hmod" ] || fail "Missing $hmod in the repository; use a release archive or run scripts/build-mod.sh."
if [ -f "$hmod.sha256" ] && has sha256sum; then
    expected=$(awk '{print $1}' "$hmod.sha256")
    actual=$(sha256sum "$hmod" | awk '{print $1}')
    [ "$expected" = "$actual" ] || fail 'The mod file does not match its checksum; download it again.'
fi
mkdir -p "$game_home/horizon-mods"
publish_file "$hmod" "$game_home/horizon-mods/horizon_voice.hmod" 644
echo "Installed the mod in $game_home/horizon-mods/horizon_voice.hmod"

# Turn it on, once, keeping every other mod's switch. The player can switch
# it off again in Options > Mods.
mods_txt=$game_home/horizon-mods.txt
mods_temp=$(mktemp "$mods_txt.tmp.XXXXXX")
if [ -f "$mods_txt" ]; then
    grep -v '^mod\.horizon_voice:' "$mods_txt" > "$mods_temp" || true
fi
printf 'mod.horizon_voice:true\n' >> "$mods_temp"
mv -f -- "$mods_temp" "$mods_txt"
mods_temp=
echo "Enabled Horizon Voice Chat in $mods_txt"

if [ "$mode" = mod ]; then
    echo 'The mod is installed. /voice and the HUD work now; audio needs the helper:'
    echo '  run the installer again without --mod-only'
    exit 0
fi

# --- the helper ------------------------------------------------------------
echo 'Building the voice helper.'
(cd "$source_dir" && cargo build --locked --release --features client --bin horizon-voice-helper)
mkdir -p "$data_dir/bin" "$data_dir/licenses"
publish_file "$source_dir/target/release/horizon-voice-helper" "$data_dir/bin/horizon-voice-helper" 755
cp "$source_dir/LICENSE" "$data_dir/licenses/VOICE-LICENSE"

mkdir -p "$install_dir"
{
    echo '#!/usr/bin/env sh'
    echo '# horizon-voice-managed'
    echo 'set -eu'
    printf "export HORIZON_VOICE_INSTALL='%s'\n" "$data_dir"
    printf "exec '%s/bin/horizon-voice-helper' \"\$@\"\n" "$data_dir"
} > "$work/wrapper"
chmod 755 "$work/wrapper"
publish_command "$work/wrapper" "$install_dir/horizon-voice-helper"

# --- configuration ---------------------------------------------------------
mkdir -p "$config_dir"
env_file=$config_dir/helper.env
configured=no
if [ -f "$env_file" ]; then
    configured=yes
elif [ -n "${HORIZON_VOICE_CHANNELS:-}" ]; then
    [ -f "$HORIZON_VOICE_CHANNELS" ] || fail 'Missing voice channels configuration.'
    umask 077
    publish_file "$HORIZON_VOICE_CHANNELS" "$config_dir/channels.conf" 600
    {
        echo '# Automatically follows the joined Horizon server.'
        echo "HORIZON_VOICE_CHANNELS=$config_dir/channels.conf"
        echo "HORIZON_VOICE_HOME=$game_home"
    } > "$env_file"
    configured=yes
elif [ -n "${HORIZON_VOICE_SERVER:-}" ] && [ -n "${HORIZON_VOICE_KEY:-}" ]; then
    umask 077
    {
        echo "# Horizon Voice Chat helper. Keep this file private: the key is a secret."
        echo "HORIZON_VOICE_SERVER=$HORIZON_VOICE_SERVER"
        echo "HORIZON_VOICE_KEY=$HORIZON_VOICE_KEY"
        echo "HORIZON_VOICE_HOME=$game_home"
    } > "$env_file"
    configured=yes
else
    umask 077
    {
        echo "# Horizon Voice Chat helper. Fill these in, then:"
        echo '#   systemctl --user restart horizon-voice-helper'
        echo 'HORIZON_VOICE_SERVER='
        echo 'HORIZON_VOICE_KEY='
        echo "HORIZON_VOICE_HOME=$game_home"
    } > "$env_file"
fi

# --- user service ----------------------------------------------------------
unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
if [ "$service" = yes ] && has systemctl; then
    mkdir -p "$unit_dir"
    {
        echo '[Unit]'
        echo 'Description=Horizon Voice Chat helper'
        echo 'Documentation=https://github.com/ESHAYAT102/horizon-voice-chat'
        echo
        echo '[Service]'
        echo 'Type=simple'
        printf 'ExecStart="%s/horizon-voice-helper" serve\n' "$install_dir"
        echo 'Restart=on-failure'
        echo 'RestartSec=2'
        echo
        echo '[Install]'
        echo 'WantedBy=default.target'
    } > "$unit_dir/horizon-voice-helper.service"
    systemctl --user daemon-reload 2>/dev/null || true
    if [ "$configured" = yes ]; then
        systemctl --user enable --now horizon-voice-helper 2>/dev/null \
            || echo 'Could not start the service now; it starts at your next login.'
    else
        echo "Set the relay address and key in $env_file, then:"
        echo '  systemctl --user enable --now horizon-voice-helper'
    fi
else
    echo "Start the helper yourself with: $install_dir/horizon-voice-helper serve"
fi

cat <<EOF

Installed.
  Mod      $game_home/horizon-mods/horizon_voice.hmod (enabled in Options > Mods)
  Helper   $install_dir/horizon-voice-helper
  Config   $env_file

In game: /voice toggles the microphone, /voice status shows the room.
Hotkey:  bind a desktop key to "horizon-voice-helper toggle", for example in
         Hyprland: bind = , V, exec, horizon-voice-helper toggle

Uninstall with scripts/uninstall.sh; it removes the mod, the helper and the
service and leaves your worlds, options and stock Horizon untouched.
EOF
case ":$PATH:" in *":$install_dir:"*) ;; *) echo "Add $install_dir to PATH.";; esac
