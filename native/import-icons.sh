#!/bin/sh
# Import assets from the user's own Simple Voice Chat JAR, never its Java code.
set -eu
[ "$#" -ge 1 ] && [ "$#" -le 2 ] || { echo 'Usage: native/import-icons.sh /path/to/voicechat.jar [game-home]' >&2; exit 2; }
jar=$1
home=${2:-${HORIZON_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/horizon}}
test -f "$jar" && test -d "$home" || { echo 'Existing mod JAR and game home required.' >&2; exit 1; }
command -v unzip >/dev/null
temp=$(mktemp -d "$home/.voice-icons.XXXXXX")
trap 'rm -rf "$temp"' EXIT
for name in microphone microphone_off disconnected; do
    unzip -p "$jar" "assets/voicechat/textures/icons/$name.png" > "$temp/$name.png"
    test "$(od -An -tx1 -N8 "$temp/$name.png" | tr -d ' \n')" = 89504e470d0a1a0a
    test "$(od -An -tx1 -j16 -N8 "$temp/$name.png" | tr -d ' \n')" = 0000001000000010 \
        || { echo 'Expected 16x16 voice icons.' >&2; exit 1; }
done
destination=$home/horizon-voice-icons
mkdir -p "$destination"
for name in microphone microphone_off disconnected; do
    if [ -f "$destination/$name.png" ] && ! cmp -s "$destination/$name.png" "$temp/$name.png"; then
        cp -p "$destination/$name.png" "$destination/$name.png.before-import.$(date +%s).$$"
    fi
    chmod 644 "$temp/$name.png"
    mv "$temp/$name.png" "$destination/$name.png"
done
cat > "$destination/ATTRIBUTION.txt" <<'EOF'
Simple Voice Chat icon textures by Max Henkel / the Simple Voice Chat project.
Extracted locally from the user's installed mod. Not part of the Horizon engine patch.
The source mod declares All Rights Reserved. No redistribution license is implied.
https://modrepo.de/minecraft/voicechat/overview
EOF
echo 'Imported the exact original icon PNGs. Restart the client to load them.'
