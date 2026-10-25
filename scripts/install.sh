#!/usr/bin/env sh
set -eu

# Clone, build and install per user; never silently install dependencies.
repo_url=${HORIZON_VOICE_REPO:-https://github.com/ESHAYAT102/horizon-voice-chat.git}
install_dir=${XDG_BIN_HOME:-$HOME/.local/bin}
data_dir=${XDG_DATA_HOME:-$HOME/.local/share}/horizon-voice
source_dir=${HORIZON_VOICE_SOURCE:-}
mode=both
work=
publish_temp=
has() { command -v "$1" >/dev/null 2>&1; }
fail() { echo "error: $*" >&2; exit 1; }
usage() { echo 'usage: install.sh [--client-only | --relay-only]'; }
while [ "$#" -gt 0 ]; do
    case "$1" in
        --client-only) mode=client;;
        --relay-only) mode=relay;;
        -h|--help) usage; exit 0;;
        *) usage >&2; fail "unknown option: $1";;
    esac
    shift
done
case $(uname -s) in Linux) ;; *) fail 'This first release supports Linux only.';; esac
for path in "$install_dir" "$data_dir"; do
    case "$path" in /*) ;; *) fail 'Install paths must be absolute.';; esac
    case "$path" in *"'"*|*'
'*) fail 'Install paths must not contain quotes or newlines.';; esac
done
cleanup() {
    [ -z "$publish_temp" ] || rm -f -- "$publish_temp"
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
missing=0
for tool in git cargo rustc cc pkg-config; do has "$tool" || missing=1; done
if [ "$mode" != relay ]; then
    pkg-config --exists alsa libudev 2>/dev/null || missing=1
fi
if [ "$missing" = 1 ]; then
    confirm 'Install the required build tools and development headers?' || fail 'Install Rust, git, a C compiler and pkg-config (plus ALSA/udev headers for the client), then retry.'
    if has pacman; then
        # Never use pacman -Sy: partial upgrades are unsafe on Arch.
        if [ "$mode" = relay ]; then root pacman -S --needed rust git base-devel;
        else root pacman -S --needed rust git base-devel alsa-lib systemd-libs; fi
    elif has apt-get; then
        root apt-get update
        if [ "$mode" = relay ]; then root apt-get install -y cargo rustc git build-essential pkg-config;
        else root apt-get install -y cargo rustc git build-essential pkg-config libasound2-dev libudev-dev; fi
    elif has dnf; then
        if [ "$mode" = relay ]; then root dnf install -y cargo rust git gcc pkgconf-pkg-config;
        else root dnf install -y cargo rust git gcc pkgconf-pkg-config alsa-lib-devel systemd-devel; fi
    else fail 'Supported dependency installers: pacman, apt-get, dnf. Install prerequisites manually.'; fi
fi
cargo --version >/dev/null || fail 'Cargo is unavailable; configure your Rust toolchain first.'
rustc --version >/dev/null || fail 'Rust is unavailable; configure your Rust toolchain first.'
rust_version=$(rustc --version | awk '{print $2}')
printf '%s\n' "$rust_version" | awk -F. '{ exit !($1 > 1 || ($1 == 1 && $2 >= 90)) }' || fail "Rust 1.90 or newer is required (found $rust_version). Update your toolchain and retry."
mkdir -p "$install_dir" "$data_dir/bin"
for name in horizon-voice horizon-voice-relay; do
    if [ -e "$install_dir/$name" ]; then
        grep -q '^# horizon-voice-managed$' "$install_dir/$name" || fail "Refusing to overwrite an unrelated $install_dir/$name"
    fi
done
work=$(mktemp -d "${TMPDIR:-/tmp}/horizon-voice.XXXXXX")
if [ -z "$source_dir" ]; then
    echo "Cloning $repo_url"
    git clone --depth 1 "$repo_url" "$work/source"
    source_dir=$work/source
fi
source_dir=$(CDPATH= cd -- "$source_dir" && pwd)
publish_file() {
    # Rename, rather than overwrite, so reinstalling a running binary is safe.
    publish_temp=$(mktemp "$2.tmp.XXXXXX")
    cp "$1" "$publish_temp"
    chmod "$3" "$publish_temp"
    mv -f -- "$publish_temp" "$2"
    publish_temp=
}
install_wrapper() {
    name=$1
    target=$2
    {
        echo '#!/usr/bin/env sh'
        echo '# horizon-voice-managed'
        echo 'set -eu'
        printf "export HORIZON_VOICE_INSTALL='%s'\n" "$data_dir"
        printf "exec sh '%s/%s' \"\$@\"\n" "$data_dir" "$target"
    } > "$work/wrapper"
    chmod 755 "$work/wrapper"
    publish_file "$work/wrapper" "$install_dir/$name" 755
}
if [ "$mode" != client ]; then
    echo 'Building the voice relay (no game or microphone dependencies).'
    (cd "$source_dir" && cargo build --locked --release --bin horizon-voice-relay)
    publish_file "$source_dir/target/release/horizon-voice-relay" "$data_dir/bin/horizon-voice-relay" 755
    cp "$source_dir/scripts/run-relay.sh" "$data_dir/run-relay.sh"
    install_wrapper horizon-voice-relay run-relay.sh
fi
if [ "$mode" != relay ]; then
    revision=$(cat "$source_dir/horizon-revision")
    echo "Fetching pinned Horizon $revision (a full game build can take a long time)."
    git init -q "$work/horizon"
    git -C "$work/horizon" remote add origin https://github.com/ludovic111/horizon.git
    git -C "$work/horizon" fetch --depth 1 origin "$revision"
    git -C "$work/horizon" checkout --detach FETCH_HEAD
    git -C "$work/horizon" apply "$source_dir/patches/horizon.patch"
    mkdir -p "$work/horizon/crates/horizon-voice/src"
    cp "$source_dir/Cargo.toml" "$source_dir/Cargo.lock" "$work/horizon/crates/horizon-voice/"
    cp "$source_dir/src/"*.rs "$work/horizon/crates/horizon-voice/src/"
    (cd "$work/horizon" && cargo build --locked --profile fast -p horizon-client --bin horizon-client -j "${HORIZON_VOICE_JOBS:-2}")
    publish_file "$work/horizon/target/fast/horizon-client" "$data_dir/bin/horizon-client" 755
    mkdir -p "$data_dir/licenses"
    cp "$work/horizon/LICENSE" "$data_dir/licenses/HORIZON-LICENSE"
    cp "$work/horizon/THIRD_PARTY_NOTICES.md" "$data_dir/licenses/HORIZON-THIRD-PARTY-NOTICES.md"
    cp "$source_dir/scripts/run-client.sh" "$data_dir/run-client.sh"
    install_wrapper horizon-voice run-client.sh
fi
mkdir -p "$data_dir/examples"
mkdir -p "$data_dir/licenses"
cp "$source_dir/LICENSE" "$data_dir/licenses/VOICE-LICENSE"
cp "$source_dir/config/"*.example "$data_dir/examples/"
echo "Installed in $data_dir. Existing Horizon installations, worlds, and voice configs were not changed."
echo "Copy the examples from $data_dir/examples to ~/.config/horizon-voice/ and configure them."
echo 'No service was enabled and no firewall ports were opened automatically.'
case ":$PATH:" in *":$install_dir:"*) ;; *) echo "Add $install_dir to PATH.";; esac
