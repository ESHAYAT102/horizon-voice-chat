# Native game/server voice — experimental engine patch

This is **not a launcher plugin**, a sandboxed `.hmod`, or a permanent fork.
It rebuilds an official upstream Horizon release with a small, opt-in engine
extension. Both server and players need the patched engine. Stock clients may
still join; they simply cannot use voice. No relay or helper is needed.

## Design

- The server announces `horizon:voice_v1` over its existing play connection.
- The client subscribes automatically, but starts muted. **V** toggles capture.
- Voice is PCM16 mono, 16 kHz, in 20 ms frames. The server stamps the sender's
  game entity id; clients cannot claim another speaker's identity.
- Only joined, voice-subscribed sessions send/receive audio. The server checks
  exact frame sizes and caps each sender at 60 frames/second. No self echo.
- Leaving drops the audio worker. Losing focus or opening a menu mutes capture.
- All audio is server-wide, across dimensions, not proximity chat. No Opus,
  noise suppression or echo cancellation; use headphones.
- Audio travels over the **game's TCP connection**, so loss/congestion can delay
  both game traffic and voice. This prototype prioritizes session integration,
  not the low-latency performance of a dedicated UDP voice protocol.
- No extra encryption is added: offline-mode game connections must use
  Tailscale or another encrypted tunnel. The server operator can hear voice.
  This does not improve offline-mode player authentication.

## Build without changing an installation

Linux build tools and Horizon's normal audio/graphics development dependencies
are required. Use a working Rust toolchain (the installer does not install packages).

```sh
sh native/build.sh v0.2.66 client
sh native/build.sh v0.2.66 server
```

Only **v0.2.66** is currently targeted; validation results are in `TESTING.md`.
Other releases are refused before installation. A new Horizon release needs an
updated/tested patch, not a permanently pinned game. No promise of compatibility
with future upstream releases is made. No upstream repository is forked.

## Explicit installation (after testing)

**Stop the target game/server first.** Pass the actual regular executable, not a
symlink such as a server runtime's `current` directory entry. Each installation
is backed up separately, and the script creates a `horizon-voice.enabled` marker
in that game's own home. Nothing edits or replaces the launcher.

```sh
sh native/install.sh /absolute/path/horizon-client /absolute/path/game-home v0.2.66 client
sh native/install.sh /absolute/path/horizon-engine /absolute/path/server-home v0.2.66 server
```

Do not enable the old companion helper at the same time: disable its user service
and the `horizon_voice` HUD mod when moving to native voice, after testing.
These scripts do not automatically remove the existing working setup.

An upstream launcher/server update can replace the patched binary. The marker
does not lock updates; voice then needs a patch tested for the new release.

## Uninstall

```sh
sh native/uninstall.sh /absolute/path/game-binary
```

Restores the backup only if the current binary exactly matches what this installer
installed. An upstream-updated or otherwise modified binary is never overwritten.
Backups are retained for recovery. Worlds, options, launcher and other mods remain
untouched. `HORIZON_NATIVE_VOICE=0` also overrides the enable marker.

## Validation

`probe.rs` is a development-only protocol-bot test. Copy it into the patched
checkout's `crates/horizon-client/examples/native_voice_probe.rs`, build that
example, and run it with a scratch dedicated server's port. Never point the
probe at a real player world. It checks announcement, subscriptions, forwarding,
malformed and unsubscribed senders, no self echo and flood limits without audio.

Full human-audible multi-player testing is still required before deployment.

`probe_audio.rs` is a separate developer-only device smoke test. It opens the
default input/output devices but stays muted and sends nothing. Build it as a
`native_voice_audio_probe` example in the patched checkout to check device startup.
