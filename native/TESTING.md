# Native prototype validation

Target: upstream `v0.2.66`, commit
`93e3096e0688cc0e5e3c52e71e666246627d2207`, Linux x86-64.

- Client and control binary built with `--profile fast --features mods`.
- Client Clippy (`--lib --bin horizon-client --features mods -D warnings`): pass.
- No-audio server Clippy (`--no-default-features --features mods`): pass.
- Headless server binary built with `--no-default-features --features mods`;
  the same three-bot server test passed using the installed-style enable marker.
- Filtered Cargo wire tests also passed under `--profile fast --no-default-features
  --features mods` (1 passed; unrelated library tests filtered out).
- Standalone Rust wire tests: sender stamping, exact frame sizes, malformed
  input, 60-frame rate limit and reset: pass.
- Real scratch dedicated server + three protocol bots: pass. Checked server
  announcement, client subscriptions, forwarding between subscribed players,
  refusal of an unsubscribed sender, no forwarding to an unsubscribed listener,
  malformed-frame rejection, no self-echo and 60/100 flood-frame limit.
- Installer tests with real patch application and fake compiler output: pass.
  Unsupported versions leave the target intact; install/uninstall/reinstall
  restores the original; uninstall preserves an upstream-changed binary;
  original backups are retained.
- Explicit prebuilt deployment: checksum mismatch is rejected; verified artifact
  install/restore passes. A real glibc-incompatible artifact was rejected on a
  destination host before changing its running engine or creating an enable marker.
- A server binary linked for glibc 2.42 using Zig passed the three-bot scratch
  server test on both the build host and the Ubuntu destination. No destination
  system libraries were changed. The destination's v0.2.66 server then restarted
  successfully with the native engine installed (20 TPS, zero crashes at check).
- Native audio worker smoke test using this machine's real input/output devices:
  pass. Started muted, produced no outgoing frames and dropped cleanly. This was
  a standalone device probe, not a game run or audible two-player test.

The protocol test used synthetic PCM bytes, not an audible conversation.
Human-audible two-player voice, in-game mic/HUD interaction, packet-loss and
congestion behaviour, and Windows/macOS remain untested. Initial prototype tests
did not replace a live installation; the subsequent requested rollout installed
the native client and two server engines with backups, and disabled the old helper
and voice relays. One server remains stopped, preserving its prior running state.

A filtered full Cargo library test invocation was stopped by the execution
timeout while compiling dependencies; it is not claimed as a passing full suite.
The focused wire tests were also built and run directly with `rustc --test`.
