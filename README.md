# Horizon Voice Chat

Toggle-to-talk voice chat for [Horizon](https://github.com/ludovic111/horizon), installed **on top of an existing Horizon** as a mod plus a small companion helper. Nothing replaces Horizon, and uninstalling leaves the game exactly as it was.

It is a clean-room Horizon implementation in the spirit of [Simple Voice Chat](https://modrinth.com/plugin/simple-voice-chat) by Max Henkel, with no code from it. This is an **unofficial port for Horizon**, not a Fabric/NeoForge mod and not a Simple Voice Chat protocol implementation.

Press **V** to turn the microphone on; press it again to mute. The microphone starts muted every time. Everyone in the same voice room hears everyone else, regardless of distance or dimension.

## How it is put together

Horizon mods run in a WebAssembly sandbox that has no microphone, audio output, sockets or raw input, by design. So the install has two halves:

| Part | What it does | What it may access |
| --- | --- | --- |
| `horizon_voice.hmod` | `/voice`, the `MIC ON` / `MUTED` indicator, the room count | only `hud` and `storage`; no microphone, no network |
| `horizon-voice-helper` | microphone capture, playback, relay transport, a desktop hotkey | your audio devices and one UDP socket |
| `horizon-voice-relay` | forwards authenticated packets between players | one UDP port on the host |

The mod and the helper never talk over a socket. They exchange two small `key=value` files in the mod's storage folder (`<game home>/horizon-mods/.horizon-mod-data/horizon_voice/`): `control` says whether the microphone should be live, and `status` reports connection, microphone state, room size and errors. The mod treats a status file that stops updating as a stopped helper, so a killed process cannot leave a stale `MIC ON` on screen.

## Requirements

**Every player:** Linux, Horizon **v0.2.58 or newer** with its mod runtime, a microphone and headphones, and Tailscale for the relay link. Rust is only needed when building from source; the installer does that for you.

**The voice host:** any always-on Linux machine on the same tailnet. It does not need a GPU or Minecraft files.

Horizon's own microphone/network interfaces are not exposed to mods, so the helper is unavoidable today. If Horizon ever grows a voice capability, the helper can shrink to a transport.

## Install

One line, on each player's desktop:

```sh
curl -fsSL https://raw.githubusercontent.com/ESHAYAT102/horizon-voice-chat/main/scripts/install.sh | sh
```

From a clone:

```sh
git clone https://github.com/ESHAYAT102/horizon-voice-chat.git
cd horizon-voice-chat
sh scripts/install.sh
```

What it does:

1. Copies `dist/horizon_voice.hmod` into `<game home>/horizon-mods/` (checksum verified).
2. Adds `mod.horizon_voice:true` to `<game home>/horizon-mods.txt`, keeping every other mod's line.
3. Builds `horizon-voice-helper` and puts a small wrapper in `~/.local/bin`.
4. Writes a private `~/.config/horizon-voice/helper.env` (mode 600).
5. Installs and starts a systemd **user** service, `horizon-voice-helper.service`.

Options:

```sh
sh scripts/install.sh --mod-only    # only the .hmod: /voice and the HUD, no audio
sh scripts/install.sh --no-service  # build and configure the helper, do not enable the service
sh scripts/install.sh --relay-only  # the voice host only
```

Missing build tools are installed only after an explicit prompt. The installer never opens firewall ports.

Paths:

| Purpose | Default |
| --- | --- |
| Mod | `<game home>/horizon-mods/horizon_voice.hmod` |
| Commands | `~/.local/bin/horizon-voice-helper`, `~/.local/bin/horizon-voice-relay` |
| Programs | `~/.local/share/horizon-voice/bin/` |
| Private configuration | `~/.config/horizon-voice/` |
| User service | `~/.config/systemd/user/horizon-voice-helper.service` |

`XDG_BIN_HOME`, `XDG_DATA_HOME`, `XDG_CONFIG_HOME` and `HORIZON_HOME` are respected. The game home defaults to `~/.local/share/horizon`, the folder holding `options.txt`.

## Configure the relay (host)

Join the same tailnet on the host and every player. Get the host's address with `tailscale ip -4`, then:

```sh
sh scripts/install.sh --relay-only
mkdir -p ~/.config/horizon-voice
cp ~/.local/share/horizon-voice/examples/relay.env.example ~/.config/horizon-voice/relay.env
chmod 600 ~/.config/horizon-voice/relay.env
```

Edit `relay.env`:

```sh
export HORIZON_VOICE_KEY='YOUR_GENERATED_64_CHARACTER_HEX_KEY'
export HORIZON_VOICE_BIND='YOUR_TAILSCALE_IP:24454'
export HORIZON_VOICE_LIMIT='2'
```

Generate the key with `openssl rand -hex 32` and give the same value to every player. Start the relay with `horizon-voice-relay`, under your own supervisor or a service. If UFW is active, allow UDP **only** on Tailscale:

```sh
sudo ufw allow in on tailscale0 to any port 24454 proto udp
```

The relay refuses public and wildcard bind addresses on purpose. Do not expose it to the internet.

## Configure each player

### Automatic per-server channels (Linux)

The helper can follow the server you join, without commands, a replacement client,
or a pinned Horizon build. Your administrator supplies a private `channels.conf`:

```text
# name   game-IP:port          voice-relay-IP:port       room-key
Cosmos   100.104.12.117:25565   100.104.12.117:24454       REPLACE_WITH_COSMOS_64_HEX_KEY
Homelab  100.67.77.122:25566    100.67.77.122:24455        REPLACE_WITH_HOMELAB_64_HEX_KEY
```

Keep this file private (mode 600). Add a row for each address players use
(LAN, Tailscale, IPv4 or IPv6), using the same name, relay and key for aliases.
Addresses are numeric: DNS names must be resolved by the administrator.
Each server uses a separate relay/key, so its audio never crosses into the other room.

For a new install, an administrator can pass `HORIZON_VOICE_CHANNELS=/path/to/channels.conf`
to the installer. For an existing install, set `HORIZON_VOICE_CHANNELS` in
`helper.env` to the absolute path of the private channels file, then restart
the helper once. Players then only join a game server normally.

The helper checks established TCP connections owned by the desktop `horizon-client`
through Linux `/proc`. Dedicated-server processes are ignored. It opens audio only
for a configured connection, leaves voice on disconnect, and mutes on every switch.
Unknown servers, unreadable connection information and conflicting concurrent sessions
leave voice disconnected. Detection is polled every 200 ms, not an in-game join event;
restricted `/proc`, renamed executables, proxies and Friends tunnels are not supported.
This is automatic selection of administrator-provisioned channels, not automatic discovery
of arbitrary servers or distribution of secret keys. Microphone toggling is still explicit.

The installer asks for nothing; edit `~/.config/horizon-voice/helper.env`:

```sh
HORIZON_VOICE_SERVER=100.x.y.z:24454
HORIZON_VOICE_KEY=THE_SAME_64_CHARACTER_KEY
HORIZON_VOICE_HOME=/home/you/.local/share/horizon
```

Then restart the helper:

```sh
systemctl --user restart horizon-voice-helper
horizon-voice-helper status
```

If the installer was run without a service, start it yourself with `horizon-voice-helper serve`.

## Use

In game, type:

```text
/voice            toggle the microphone
/voice on         microphone on
/voice off        mute
/voice status     connection, microphone state and room size
```

The bottom-left indicator shows `Voice: MUTED`, `Voice: MIC ON`, or `Voice: MIC ON · 2 others`. It says `Voice: helper not running` when the helper is not up. Start Horizon (or restart it) after installing, so the new mod is loaded.

The sandbox cannot see keyboard shortcuts, so **V** is a desktop shortcut that flips the same control file:

- **Hyprland:** add to `~/.config/hypr/hyprland.conf`:
  ```ini
  bind = , V, exec, horizon-voice-helper toggle
  ```
  If V is already a game bind, use a modifier, for example `bind = SUPER, V, ...`.
- **GNOME:** Settings > Keyboard > Custom Shortcuts, command `horizon-voice-helper toggle`.
- **KDE:** System Settings > Shortcuts > Custom Shortcuts, command `horizon-voice-helper toggle`.

`/voice` always works and is the reliable fallback.

## Uninstall

```sh
sh scripts/uninstall.sh
```

Or:

```sh
curl -fsSL https://raw.githubusercontent.com/ESHAYAT102/horizon-voice-chat/main/scripts/uninstall.sh | sh
```

It stops and removes the user service, the mod, the mod's storage folder, the `mod.horizon_voice` switch line, the helper, its wrapper and `helper.env` (which holds the room key). Worlds, options, other mods, resource packs and the stock Horizon installation are left untouched. Commands that were not installed by this project are never overwritten or deleted.

## Privacy and limitations

- No recording, no telemetry. Microphone samples are dropped while muted.
- The microphone starts muted at every helper start and every game start. It also mutes on relay timeout.
- The room key authenticates packets; it is **not encryption**. Use Tailscale for encryption and access control. Anyone with the key and tailnet access can join.
- HMAC-SHA256, bounded packet sizes, replay rejection for live sessions, a per-peer rate cap and a peer limit. Not hardened for a hostile public network.
- Mono 16 kHz PCM in 20 ms packets (about 256 kbit/s per talking player), not Opus. No echo cancellation, noise suppression, proximity audio or Simple Voice Chat compatibility.
- Use headphones: an open microphone can feed other players' voices back into the room. The default OS microphone and speakers are used; restart the helper after changing devices.
- Linux only for now. The relay and helper are portable Rust; the installer and service integration are Linux-specific.

## Development

Build the mod (needs `cargo-horizon-mod` from a Horizon checkout):

```sh
cargo install --path <horizon>/crates/cargo-horizon-mod
sh scripts/build-mod.sh        # writes dist/horizon_voice.hmod and its .sha256
```

Checks:

```sh
cargo test --locked --features client
cargo clippy --locked --all-targets --features client -- -D warnings
sh scripts/test-install.sh     # isolated install/uninstall, throwaway XDG folders
```

`scripts/test-install.sh` builds the helper, installs into temporary folders, and checks that reinstall, uninstall, preservation of other mods/worlds and the ownership guard all work.

The mod targets the Horizon mod API `0.1` and is pinned to upstream commit `6f8cad0020d77f1bfcc3c869019c0e0efa21f683` in `mod/Cargo.toml`. A local `mod/.cargo/config.toml` (gitignored) can point it at a Horizon checkout for development. Test game builds only with isolated `HORIZON_HOME`, `HORIZON_LAUNCHER_HOME`, `XDG_DATA_HOME` and `XDG_CONFIG_HOME`; see [TESTING.md](TESTING.md).

## Credits and license

MIT; see [LICENSE](LICENSE). Horizon is MIT; its notice is kept in [HORIZON-LICENSE](HORIZON-LICENSE).

- Horizon, its mod SDK and its mod API: the Horizon contributors.
- Simple Voice Chat: Max Henkel. This project follows the same toggle-to-talk idea but shares no code and no protocol with it.
- Minecraft is a trademark of Mojang Studios. This is not an official Minecraft product and is not associated with Mojang, Microsoft or Horizon's maintainers.
