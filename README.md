# Horizon Voice Chat

Server-wide voice chat for [Horizon](https://github.com/ludovic111/horizon), with a **toggle microphone** instead of push-to-talk.

Press **V** to turn your microphone on. Press **V** again to mute. Everyone in the same voice room can hear each other, regardless of distance or dimension.

> **Experimental, Linux-first.** This is an independent native Horizon client patch and a small relay, **not** a Fabric/Forge plugin, `.hmod`, or an implementation of Simple Voice Chat's protocol. Stock Horizon does not expose the microphone/network hooks a voice mod needs. Every player needs the custom client. No Minecraft code or assets are included.

## Features

- Microphone starts muted on every connection.
- Persistent `MIC ON` / `MUTED` indicator, including with the normal HUD hidden.
- Automatically mutes on lost window focus, disconnect, or relay timeout. Returning to the game does not unmute automatically.
- Voice capture and playback run off the render thread, with bounded queues.
- Small standalone relay; the existing Horizon game server needs no binary replacement.
- Room key authenticates packets; Tailscale encrypts traffic. The relay refuses public/wildcard bind addresses.
- Two voice clients by default; configurable up to 32. The game server's player limit is separate.

## Requirements

**Relay:** Linux, Tailscale, Rust/Cargo and git to build. No Java, GPU, or Minecraft files needed for the relay.

**Client:** Linux, a microphone and headphones, Tailscale, a Vulkan-capable GPU, Rust **1.90+**, git, C build tools, pkg-config, ALSA and udev headers. A stock Horizon installation with prepared Minecraft **26.3** resources is required. You must own Minecraft and prepare it using Horizon's normal process.

The custom client is pinned to the upstream commit in [`horizon-revision`](horizon-revision), not whatever `main` happens to contain tomorrow. Upstream updates may need a new patch and retesting. Budget several GB of disk and a lengthy first compilation; do not build the full client on a low-memory game server.

## Install

Like [Skater](https://github.com/ESHAYAT102/skater), the shell installer clones the repository, builds from source, installs into your user directories, and cleans up its temporary checkout. Missing build packages require explicit confirmation. It never silently installs Rust, opens firewall ports, or enables services.

Run from a clone (works before this repository is published):

```sh
git clone https://github.com/ESHAYAT102/horizon-voice-chat.git
cd horizon-voice-chat

# On the machine hosting voice:
HORIZON_VOICE_SOURCE="$PWD" sh scripts/install.sh --relay-only

# On each player's desktop:
HORIZON_VOICE_SOURCE="$PWD" sh scripts/install.sh --client-only
```

Once published at that address, the Skater-style one-line install is:

```sh
curl -fsSL https://raw.githubusercontent.com/ESHAYAT102/horizon-voice-chat/main/scripts/install.sh | sh -s -- --client-only
```

Replace `--client-only` with `--relay-only` for the host, or omit it to build both. Linux dependency installation supports apt, pacman and dnf; other distributions can install the prerequisites manually. Windows/macOS installers are not provided yet.

Paths:

| Purpose | Default |
| --- | --- |
| Commands | `~/.local/bin/horizon-voice`, `~/.local/bin/horizon-voice-relay` |
| Programs | `~/.local/share/horizon-voice/bin/` |
| Private configuration | `~/.config/horizon-voice/` |
| Custom client's worlds/options | `~/.local/share/horizon-voice/game/` |

`XDG_BIN_HOME`, `XDG_DATA_HOME`, and `XDG_CONFIG_HOME` are respected. Installation does not replace `horizon-launcher` or modify stock player data. Add the command directory to `PATH` if necessary.

## Configure the relay

Join the same Tailscale network on the server and every player's desktop. Get the server's address with `tailscale ip -4`.

```sh
mkdir -p ~/.config/horizon-voice
chmod 700 ~/.config/horizon-voice
cp config/relay.env.example ~/.config/horizon-voice/relay.env
chmod 600 ~/.config/horizon-voice/relay.env
```

Generate a room key with `openssl rand -hex 32`, then edit `relay.env`:

```sh
export HORIZON_VOICE_KEY='YOUR_GENERATED_64_CHARACTER_HEX_KEY'
export HORIZON_VOICE_BIND='YOUR_TAILSCALE_IP:24454'
export HORIZON_VOICE_LIMIT='2'
```

Start it:

```sh
horizon-voice-relay
```

If UFW is active, explicitly allow UDP **only** on Tailscale:

```sh
sudo ufw allow in on tailscale0 to any port 24454 proto udp
```

No router forwarding is needed. Do not expose this UDP service publicly. The installer intentionally makes no firewall or service changes. Run it under your preferred service supervisor for persistence; otherwise closing the terminal stops it.

## Configure each client

```sh
mkdir -p ~/.config/horizon-voice
chmod 700 ~/.config/horizon-voice
cp config/client.env.example ~/.config/horizon-voice/client.env
chmod 600 ~/.config/horizon-voice/client.env
```

Set the same room key and the relay's Tailscale address. `HORIZON_VOICE_GAME_SERVERS` must list the **exact addresses you enter in Multiplayer**, with commas and no spaces:

```sh
export HORIZON_VOICE_KEY='SAME_KEY_AS_THE_RELAY'
export HORIZON_VOICE_SERVER='YOUR_TAILSCALE_IP:24454'
export HORIZON_VOICE_GAME_SERVERS='YOUR_TAILSCALE_IP:25566,homelab:25566'
export HORIZON_VOICE_PLAYER='YourPlayerName'
```

The launcher wrapper finds prepared Minecraft 26.3 resources in the stock Horizon launcher's versions directory. If needed, specify them with `HORIZON_VOICE_RESOURCES` in `client.env` (see the example). Prepared data is read from there; worlds/options use the separate voice-client folder.

```sh
horizon-voice
```

Join the configured game server. Wait for `Voice: MUTED [V]`, then press **V**. Hold-to-talk is not required, and key repeats do not repeatedly flip the microphone. Each player must do this separately.

## Uninstall

```sh
sh scripts/uninstall.sh
```

Or, after publication:

```sh
curl -fsSL https://raw.githubusercontent.com/ESHAYAT102/horizon-voice-chat/main/scripts/uninstall.sh | sh
```

The uninstaller removes only voice-owned commands and programs. It preserves private configuration and separate client worlds. `--purge` additionally removes `client.env` and `relay.env`, **not worlds**. Stock Horizon, the game server, Minecraft resources, and system dependencies stay untouched.

If you manually created a service, stop and remove it first. Remove a manually added UFW rule with:

```sh
sudo ufw delete allow in on tailscale0 to any port 24454 proto udp
```

## Privacy and limitations

- No recording or telemetry in the voice code. Microphone capture is discarded while muted. Audio device streams are opened when joining a configured server, even while muted; your desktop may show a microphone-in-use indicator.
- The shared key is a **room credential**, not Microsoft authentication. The relay does not validate game-server membership; the normal client gates voice on its joined-server state. Anyone with the key and tailnet access can write a compatible client. Share only with trusted players; rotate the key after removing someone. All room members can hear all room audio.
- HMAC-SHA256 provides integrity/authentication; it is **not encryption**. Always use Tailscale for encryption. The UDP relay has bounded peer counts, packet sizes, replay checks for live sessions, and a per-peer rate cap. It is not hardened for hostile public networks.
- Mono 16 kHz PCM in 20 ms packets (~256 kbit/s per talking player, plus overhead), not Opus. No echo cancellation, noise suppression, configurable keybind, automatic game-server discovery, proximity audio or compatibility with Simple Voice Chat.
- Use headphones: an open microphone can feed other players' voices back into the room. The default OS microphone and speakers are used; restart the client after changing devices.
- `Voice connecting` means no recent relay acknowledgement: check key, Tailscale, UDP firewall, and the room limit. It stays muted until a connection is confirmed. An audio-device failure requires reconnecting to the game server.
- Separate client data means your stock username/options are not copied automatically. Set the same player name to match your server whitelist.

## Development

Relay/protocol/audio-client checks (no game data or microphone access):

```sh
cargo test --locked --features client
cargo clippy --locked --all-targets --features client -- -D warnings
```

The installer fetches the pinned Horizon revision, applies `patches/horizon.patch`, copies this Rust crate into `crates/horizon-voice`, and builds `horizon-client` with its normal audio feature. The patch introduces no dependency on Minecraft's Java mod loaders. Test game builds only with isolated `HORIZON_HOME`, `HORIZON_LAUNCHER_HOME`, `XDG_DATA_HOME`, and `XDG_CONFIG_HOME`, and `HORIZON_NO_AUDIO=1` for automated tests. Real microphone tests require explicit permission and both players.

Current implementation is experimental: compile/unit/transport checks are not proof of an audible, two-player end-to-end test. See [TESTING.md](TESTING.md) for verified checks and remaining gaps before distributing a release.

## License and credits

MIT; see [LICENSE](LICENSE). The client is a modified build of Horizon, also MIT; its original notice is in [HORIZON-LICENSE](HORIZON-LICENSE). Generated client distributions must retain that notice and applicable dependency licenses. No code was copied from Simple Voice Chat. Not an official Minecraft product; not associated with Mojang, Microsoft, or Horizon's maintainers.
