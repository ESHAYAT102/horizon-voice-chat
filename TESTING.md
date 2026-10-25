# Verification

Local checks, 2026-10-26, Linux x86-64:

- `cargo test --locked --features client`: **14 tests pass** (12 unit tests, including the helper's file protocol and its single-instance lock, and 2 relay integration/CLI tests).
- `cargo clippy --locked --all-targets --features client -- -D warnings`: passes.
- Shell syntax checks: pass for all scripts.
- `scripts/build-mod.sh` builds `dist/horizon_voice.hmod` (12,754 bytes) and writes its SHA-256.
- `scripts/test-install.sh`: mod-only install, full install with `--no-service`, reinstall (one switch line), uninstall, repeated uninstall, preservation of another mod, a world file and unrelated commands, and the ownership guard all pass in temporary XDG directories.
- The user service unit is generated with an absolute, quoted `ExecStart` and `WantedBy=default.target`; enabling was exercised with a throwaway `XDG_CONFIG_HOME`, so the unit was written and removed but not started by the real user manager.
- Stock Horizon **v0.2.58** with its own prepared 26.3 resources, isolated `HORIZON_HOME` and `XDG_*`:
  - The `.hmod` appears in Options > Mods as "Horizon Voice Chat 0.2.0: ON" and loads with only `hud` and `storage`.
  - `/voice on` and `/voice off` write `transmitting=1` / `transmitting=0` to the control file and answer in chat.
  - The HUD shows `Voice: MIC ON`, `Voice: MUTED` and `Voice: MIC ON · 1 other` from a real helper status file; screenshots were checked.
  - `/voice status` reports the helper's state, and `helper not running` when the status is absent.
- Helper + relay, real audio devices (PipeWire, AMD Renoir laptop):
  - The helper connects to a loopback relay, reports `connected=1`, and starts muted even with a stale `transmitting=1` control file.
  - `/voice on` starts capture: a protocol peer received 250 audio packets in 5 s (50/s, 20 ms packets).
  - The peer count reaches 1 while another peer sends audio and returns to 0 within 3 s after it stops.
  - `horizon-voice-helper toggle`, `on`, `off` and `status` work without a game running.
- Audio playback is opened by the helper during these tests; no audible two-person conversation was judged by ear.

## Not yet verified

- An audible conversation between two people with microphones and headphones.
- A full install on a real desktop session with the systemd user service started by the user manager.
- Audio-device changes, long-running sessions, or packet-loss-heavy networks.
- Windows/macOS builds (the installer deliberately supports Linux only).
- Relay deployment on the homelab host (UDP 24454 is currently held by an unrelated process there).

Protocol tests use generated PCM and do not access a microphone. Passing them does not prove acoustic quality, echo suppression, or acceptable latency on a particular network. Headphones are recommended; this version has no echo cancellation or noise suppression.
