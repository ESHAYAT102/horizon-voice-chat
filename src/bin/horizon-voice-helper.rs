//! `horizon-voice-helper`: microphone, playback and relay transport for the
//! Horizon Voice Chat mod. Run `serve` as a user service; `toggle`, `on` and
//! `off` are for a desktop hotkey and scripts.
use horizon_voice::helper::{self, Config};
use std::process::ExitCode;

const USAGE: &str = "usage: horizon-voice-helper [serve | toggle | on | off | status | --help]

serve    connect to the relay and follow the mod's /voice control file
toggle   flip the microphone, for a desktop hotkey
on/off   set the microphone explicitly
status   print the state the mod would show

Configuration (environment, then ~/.config/horizon-voice/helper.env):
  HORIZON_VOICE_SERVER  relay Tailscale IPv4 address:UDP port
  HORIZON_VOICE_KEY     shared 64-character hexadecimal room key
  HORIZON_VOICE_HOME    Horizon game folder (default: ~/.local/share/horizon)
  HORIZON_VOICE_CHANNELS Private routes file; automatically follows the joined server
                         (overrides the fixed relay/key settings)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("serve");
    let result = match command {
        "serve" => Config::load().and_then(|config| helper::serve(&config)),
        "toggle" => with_home(|home| {
            let on = helper::toggle(home)?;
            println!("microphone {}", if on { "on" } else { "muted" });
            Ok(())
        }),
        "on" => with_home(|home| helper::set(home, true)),
        "off" => with_home(|home| helper::set(home, false)),
        "status" => with_home(|home| {
            println!("{}", helper::describe(home));
            Ok(())
        }),
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        "--version" | "-V" => {
            println!("horizon-voice-helper {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("unknown command: {other}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The home is enough for the control-file commands; they do not need a key.
fn with_home(f: impl FnOnce(&std::path::Path) -> Result<(), String>) -> Result<(), String> {
    let home = Config::home()?;
    f(&home)
}
