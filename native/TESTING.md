# Native prototype validation

Target: upstream `v0.2.66`, commit
`93e3096e0688cc0e5e3c52e71e666246627d2207`, Linux x86-64.

- Client and control binary built with `--profile fast --features mods`.
- Client Clippy (`--lib --bin horizon-client --features mods -D warnings`): pass.
- No-audio server Clippy (`--no-default-features --features mods`): pass.
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
- Native audio worker smoke test using this machine's real input/output devices:
  pass. Started muted, produced no outgoing frames and dropped cleanly. This was
  a standalone device probe, not a game run or audible two-player test.

The protocol test used synthetic PCM bytes, not an audible conversation.
Human-audible two-player voice, in-game mic/HUD interaction, packet-loss and
congestion behaviour, Windows/macOS, and live server deployment remain untested.
The existing client/helper/relays were not replaced during these tests.

A filtered full Cargo library test invocation was stopped by the execution
timeout while compiling dependencies; it is not claimed as a passing full suite.
The focused wire tests were also built and run directly with `rustc --test`.
