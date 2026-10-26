#!/bin/sh
# Isolated installer test. Real upstream tree and patch, fake compiler output.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
checkout=${1:?Pass an existing upstream Horizon checkout}
work=$(mktemp -d "${TMPDIR:-/tmp}/native-voice-install.XXXXXX")
trap 'rm -rf "$work"' EXIT
export XDG_DATA_HOME="$work/data"
mkdir -p "$work/bin" "$work/game" "$XDG_DATA_HOME/horizon-voice/native/build"
source=$XDG_DATA_HOME/horizon-voice/native/build/v0.2.66-client
git clone --quiet --shared "$checkout" "$source"
git -C "$source" checkout --quiet --detach v0.2.66
cat > "$work/bin/cargo" <<'EOF'
#!/bin/sh
set -eu
mkdir -p target/fast
printf '#!/bin/sh\necho patched-test-binary\n' > target/fast/horizon-client
chmod 755 target/fast/horizon-client
EOF
chmod 755 "$work/bin/cargo"
export PATH="$work/bin:$PATH"
printf '#!/bin/sh\necho original\n' > "$work/horizon-client"
chmod 755 "$work/horizon-client"
cp -p "$work/horizon-client" "$work/original"
if sh "$root/native/install.sh" "$work/horizon-client" "$work/game" v999; then
    echo 'FAIL: unsupported release accepted' >&2; exit 1
fi
cmp "$work/horizon-client" "$work/original"
# Checksum mismatch cannot install or create the enable marker.
if HORIZON_VOICE_PREBUILT="$work/original" HORIZON_VOICE_PREBUILT_VERSION=v0.2.66 HORIZON_VOICE_PREBUILT_SHA256=bad \
    sh "$root/native/install.sh" "$work/horizon-client" "$work/game" v0.2.66 client; then
    echo 'FAIL: unverified prebuilt accepted' >&2; exit 1
fi
test ! -e "$work/game/horizon-voice.enabled"
sh "$root/native/install.sh" "$work/horizon-client" "$work/game" v0.2.66 client
test -f "$work/game/horizon-voice.enabled"
grep -q patched-test-binary "$work/horizon-client"
sh "$root/native/uninstall.sh" "$work/horizon-client"
cmp "$work/horizon-client" "$work/original"
test ! -e "$work/game/horizon-voice.enabled"
sh "$root/native/install.sh" "$work/horizon-client" "$work/game" v0.2.66 client
sh "$root/native/uninstall.sh" "$work/horizon-client"
cmp "$work/horizon-client" "$work/original"
HORIZON_VOICE_PREBUILT="$source/target/fast/horizon-client" HORIZON_VOICE_PREBUILT_VERSION=v0.2.66 \
    HORIZON_VOICE_PREBUILT_SHA256=$(sha256sum "$source/target/fast/horizon-client" | cut -d' ' -f1) \
    sh "$root/native/install.sh" "$work/horizon-client" "$work/game" v0.2.66 client
sh "$root/native/uninstall.sh" "$work/horizon-client"
cmp "$work/horizon-client" "$work/original"
# A different binary is a different installation record.
cp -p "$work/original" "$work/horizon-client-2"
sh "$root/native/install.sh" "$work/horizon-client-2" "$work/game" v0.2.66 client
printf 'upstream update\n' > "$work/horizon-client-2"
sh "$root/native/uninstall.sh" "$work/horizon-client-2"
grep -qx 'upstream update' "$work/horizon-client-2"
test ! -e "$work/game/horizon-voice.enabled"
echo 'PASS: patch application, unsupported release guard, backup/restore, changed binary preservation'
