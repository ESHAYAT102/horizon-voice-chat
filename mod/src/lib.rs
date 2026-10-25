//! Horizon Voice Chat, mod half.
//!
//! The mod sandbox has no microphone, audio output or sockets, so this half
//! only does what the sandbox allows: it registers `/voice`, draws the
//! microphone indicator, and exchanges two tiny files with the companion
//! helper through the `storage` capability.
//!
//! - `control` is written here and read by the helper: `transmitting=1`.
//! - `status` is written by the helper and read here: connection, whether the
//!   microphone is live, how many other players are in the room, and an error.
//!
//! Both are `key=value` lines so neither side needs a JSON parser.

use horizon_mod_sdk::{
    Mod, Registration,
    hud::{self, Corner, Line},
    storage,
};

const CONTROL: &str = "control";
const STATUS: &str = "status";

const GREEN: u32 = 0x55FF55;
const RED: u32 = 0xFF5555;
const GREY: u32 = 0xAAAAAA;
const YELLOW: u32 = 0xFFFF55;

/// What the helper last reported.
#[derive(Default, PartialEq, Eq)]
struct Status {
    /// The status file exists: the helper has run at least once.
    present: bool,
    connected: bool,
    transmitting: bool,
    peers: u32,
    error: String,
    /// The helper's write counter. Unchanged for 2 s means it stopped.
    beat: u64,
}

fn parse_status(text: &str) -> Status {
    let mut status = Status {
        present: true,
        ..Status::default()
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "connected" => status.connected = value.trim() == "1",
            "transmitting" => status.transmitting = value.trim() == "1",
            "peers" => status.peers = value.trim().parse().unwrap_or(0),
            "beat" => status.beat = value.trim().parse().unwrap_or(0),
            "error" => status.error = value.trim().to_owned(),
            _ => {}
        }
    }
    status
}

fn read_status() -> Status {
    storage::get(STATUS)
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map_or_else(Status::default, |text| parse_status(&text))
}

fn write_control(on: bool) -> Result<(), String> {
    let line = if on {
        "transmitting=1\n"
    } else {
        "transmitting=0\n"
    };
    storage::set(CONTROL, line.as_bytes())
        .map_err(|e| format!("Could not write the control file: {e}"))
}

struct Voice {
    /// What `/voice` last asked for. The helper's report wins on the HUD, so a
    /// helper that is not running cannot make the indicator lie.
    wanted: bool,
    last_panel: Vec<(String, u32)>,
    ticks: u64,
    last_beat: u64,
    /// Refreshes since the helper's beat last moved (one per half second).
    stale: u32,
}

impl Voice {
    fn lines(&self, status: &Status) -> Vec<(String, u32)> {
        if !status.present {
            return vec![("Voice: helper not running".into(), YELLOW)];
        }
        if !status.error.is_empty() {
            return vec![(format!("Voice: {}", status.error), RED)];
        }
        if !status.connected {
            return vec![(
                if status.transmitting {
                    "Voice: connecting — MIC ON".into()
                } else {
                    "Voice: connecting — MUTED".into()
                },
                GREY,
            )];
        }
        let others = match status.peers {
            0 => String::new(),
            1 => " · 1 other".into(),
            n => format!(" · {n} others"),
        };
        if status.transmitting {
            vec![(format!("Voice: MIC ON{others}"), GREEN)]
        } else {
            vec![(format!("Voice: MUTED{others}"), RED)]
        }
    }

    /// The helper's status, marked absent when its beat has stopped.
    fn status(&self) -> Status {
        let mut status = read_status();
        if status.present && status.beat == self.last_beat && self.stale >= 4 {
            status.present = false;
        }
        status
    }

    /// Ages the helper's beat; called twice a second from the tick.
    fn poll(&mut self) {
        let raw = read_status();
        if !raw.present {
            self.stale = 0;
        } else if raw.beat == self.last_beat {
            self.stale = self.stale.saturating_add(1);
        } else {
            self.last_beat = raw.beat;
            self.stale = 0;
        }
    }

    fn refresh(&mut self) {
        self.poll();
        let lines = self.lines(&self.status());
        if lines == self.last_panel {
            return;
        }
        let panel: Vec<Line> = lines
            .iter()
            .map(|(text, color)| Line {
                text: text.clone(),
                color: *color,
            })
            .collect();
        hud::set_panel(Corner::BottomLeft, &panel);
        self.last_panel = lines;
    }

    fn apply(&mut self, on: bool) -> Result<String, String> {
        write_control(on)?;
        self.wanted = on;
        self.refresh();
        Ok(if on {
            "Voice microphone on".into()
        } else {
            "Voice microphone muted".into()
        })
    }

    fn describe(&mut self) -> String {
        let status = self.status();
        if !status.present {
            return "Voice: the helper has not run yet. Start horizon-voice-helper.".into();
        }
        if !status.error.is_empty() {
            return format!("Voice: {}", status.error);
        }
        let state = if status.transmitting {
            "MIC ON"
        } else {
            "MUTED"
        };
        format!(
            "Voice: {state}, {}, {} other player(s)",
            if status.connected {
                "connected"
            } else {
                "connecting"
            },
            status.peers
        )
    }
}

impl Mod for Voice {
    fn init() -> Result<(Self, Registration), String> {
        // Voice always starts muted, whatever the last session ended with.
        write_control(false)?;
        Ok((
            Self {
                wanted: false,
                last_panel: Vec::new(),
                ticks: 0,
                last_beat: 0,
                stale: 0,
            },
            Registration::new().command("voice").on_tick(),
        ))
    }

    fn tick(&mut self, _tick: u64) {
        self.ticks = self.ticks.wrapping_add(1);
        // Twice a second is plenty for a status line and cheap on disk.
        if !self.ticks.is_multiple_of(10) {
            return;
        }
        self.refresh();
    }

    fn command(&mut self, name: &str, args: &str) -> Result<Option<String>, String> {
        if name != "voice" {
            return Ok(None);
        }
        match args.trim().to_ascii_lowercase().as_str() {
            "" | "toggle" => {
                // The helper's report wins: after a helper restart it may be
                // muted while this half still remembers asking for the mic.
                let status = read_status();
                let current = if status.present {
                    status.transmitting
                } else {
                    self.wanted
                };
                self.apply(!current).map(Some)
            }
            "on" | "unmute" => self.apply(true).map(Some),
            "off" | "mute" => self.apply(false).map(Some),
            "status" => Ok(Some(self.describe())),
            _ => Err("usage: /voice [on|off|toggle|status]".into()),
        }
    }
}

horizon_mod_sdk::export_mod!(Voice);
