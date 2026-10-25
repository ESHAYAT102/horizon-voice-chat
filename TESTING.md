# Verification

Local checks, 2026-10-26, Linux x86-64:

- `cargo test --locked --features client`: **8 tests pass** (6 unit tests and 2 relay integration/CLI tests).
- `cargo clippy --locked --all-targets --features client -- -D warnings`: passes.
- Shell syntax checks: passes for all scripts.
- `scripts/test-install.sh`: relay build/install, default config preservation, explicit config purge, save preservation, repeated uninstall, and refusal to overwrite an unrelated command all pass in temporary XDG directories.
- `scripts/test-wrapper.sh`: prepared-resource discovery, player-name/argument quoting, isolated game directory, and help without configuration pass using a mock game binary (no microphone access).
- The Horizon patch applies to a clean copy of the commit in `horizon-revision`.
- Horizon's public-tree check passes in the patched checkout.
- Full optimized `horizon-client` build succeeds, including a clean final rebuild of the client package (5 min 17 s). An interrupted incremental rebuild produced stale-object linker errors; cleaning only the generated client-package artifacts resolved them.
- Isolated graphical smoke test: 30 offscreen frames, Vulkan/RADV on AMD Renoir, exits 0, smoke report contains `error: null`. Temporary player/config folders, real resources read-only, microphone/audio disabled with `HORIZON_NO_AUDIO=1`.
- The final clean-build binary passes the same isolated 30-frame smoke test, exit 0, `error: null`.
- The packaged client launcher wrapper also starts correctly with a temporary configuration: 10 offscreen frames, exit 0, `error: null`.

## Not yet verified

- An audible conversation between two real microphone/speaker-equipped clients.
- Audio-device changes, long-running sessions, or packet-loss-heavy networks.
- Windows/macOS builds (the installer deliberately supports Linux only).
- End-to-end installation of the full client through a freshly cloned public GitHub repository (not published yet).

Protocol tests use generated PCM and do not access a microphone. Passing them does not prove acoustic quality, echo suppression, or acceptable latency on a particular network. Headphones are recommended; this version has no echo cancellation or noise suppression.
