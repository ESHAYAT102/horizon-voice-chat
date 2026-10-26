//! The companion helper: it owns the microphone, the speaker and the relay
//! socket, which the Horizon mod sandbox deliberately cannot reach.
//!
//! The mod and the helper never talk over the network to each other. They
//! share two small files in the mod's `storage` folder:
//!
//! - `control`, written by the mod (`/voice`), read here;
//! - `status`, written here, read by the mod for the HUD.
//!
//! Both are `key=value` lines, written through a temporary file and renamed so
//! the other side never reads half a file.

use crate::client::Client;
use crate::key_from_hex;
use std::{
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    thread,
    time::Duration,
};

/// The mod's id, which is also its `storage` folder name.
pub const MOD_ID: &str = "horizon_voice";
/// Where Horizon keeps code-mod state, beside `horizon-mods/`.
const STORAGE_ROOT: &str = ".horizon-mod-data";
const CONTROL: &str = "control";
const STATUS: &str = "status";
/// The status file is rewritten at this rate, so the HUD follows within a
/// fraction of a second without polling the disk per frame.
const STATUS_INTERVAL: Duration = Duration::from_millis(200);
const CONTROL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub struct Config {
    pub server: SocketAddr,
    pub key: [u8; 32],
    /// The game home that holds `horizon-mods/` (Horizon's `HORIZON_HOME`).
    pub home: PathBuf,
    pub channels: Option<PathBuf>,
}

impl Config {
    /// Environment first, then `~/.config/horizon-voice/helper.env`.
    fn settings() -> std::collections::HashMap<String, String> {
        let file = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("horizon-voice/helper.env"));
        let mut values = std::collections::HashMap::new();
        if let Some(file) = &file
            && let Ok(text) = std::fs::read_to_string(file)
        {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    values.insert(
                        key.trim().to_owned(),
                        value.trim().trim_matches(['"', '\'']).to_owned(),
                    );
                }
            }
        }
        values
    }
    fn get(values: &std::collections::HashMap<String, String>, name: &str) -> Option<String> {
        std::env::var(name)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| values.get(name).filter(|v| !v.is_empty()).cloned())
    }
    /// The game folder alone: enough for `toggle`, `on`, `off` and `status`.
    pub fn home() -> Result<PathBuf, String> {
        let values = Self::settings();
        Self::get(&values, "HORIZON_VOICE_HOME")
            .or_else(|| std::env::var("HORIZON_HOME").ok())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .or_else(|| {
                        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))
                    })
                    .map(|base| base.join("horizon"))
            })
            .ok_or_else(|| {
                "cannot find the Horizon game folder; set HORIZON_VOICE_HOME".to_string()
            })
    }
    pub fn load() -> Result<Self, String> {
        let values = Self::settings();
        let channels = Self::get(&values, "HORIZON_VOICE_CHANNELS").map(PathBuf::from);
        if let Some(path) = &channels {
            crate::channels::load(path)?;
            return Ok(Self {
                server: "127.0.0.1:0".parse().unwrap(),
                key: [0; 32],
                home: Self::home()?,
                channels,
            });
        }
        let server: SocketAddr = Self::get(&values, "HORIZON_VOICE_SERVER")
            .ok_or_else(|| {
                "set HORIZON_VOICE_SERVER (or write ~/.config/horizon-voice/helper.env)".to_string()
            })?
            .parse()
            .map_err(|_| "HORIZON_VOICE_SERVER is not address:port".to_string())?;
        let key = key_from_hex(&Self::get(&values, "HORIZON_VOICE_KEY").ok_or_else(|| {
            "set HORIZON_VOICE_KEY (or write ~/.config/horizon-voice/helper.env)".to_string()
        })?)?;
        Ok(Self {
            server,
            key,
            home: Self::home()?,
            channels,
        })
    }
}

/// The two files the mod and the helper share.
#[derive(Clone, Debug)]
pub struct Shared {
    dir: PathBuf,
}

impl Shared {
    pub fn new(home: &Path) -> Self {
        Self {
            dir: home.join("horizon-mods").join(STORAGE_ROOT).join(MOD_ID),
        }
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    fn control(&self) -> PathBuf {
        self.dir.join(CONTROL)
    }
    fn status(&self) -> PathBuf {
        self.dir.join(STATUS)
    }

    /// `true` when `/voice` last asked for the microphone. A missing or
    /// half-written file means muted.
    pub fn transmitting(&self) -> bool {
        std::fs::read_to_string(self.control())
            .ok()
            .and_then(|text| {
                text.lines().find_map(|line| {
                    let (key, value) = line.split_once('=')?;
                    (key.trim() == "transmitting").then(|| value.trim() == "1")
                })
            })
            .unwrap_or(false)
    }

    /// Written by `/voice` from inside the game, and by `helper toggle`.
    pub fn set_transmitting(&self, on: bool) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        write_atomic(
            &self.control(),
            if on {
                b"transmitting=1\n"
            } else {
                b"transmitting=0\n"
            },
        )
    }

    pub fn write_status(&self, status: &Status) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let mut text = String::with_capacity(96);
        text.push_str(if status.connected {
            "connected=1\n"
        } else {
            "connected=0\n"
        });
        text.push_str(if status.transmitting {
            "transmitting=1\n"
        } else {
            "transmitting=0\n"
        });
        text.push_str(&format!("peers={}\n", status.peers));
        text.push_str(&format!("beat={}\n", status.beat));
        text.push_str(&format!("error={}\n", status.error.replace('\n', " ")));
        write_atomic(&self.status(), text.as_bytes())
    }

    pub fn read_status(&self) -> String {
        std::fs::read_to_string(self.status()).unwrap_or_default()
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(tmp, path)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub connected: bool,
    pub transmitting: bool,
    pub peers: usize,
    pub error: String,
    /// Counts writes. The mod treats an unchanged beat as a stopped helper,
    /// so a killed process cannot leave a stale `MIC ON` on screen.
    pub beat: u64,
}

/// Runs until the audio worker stops. Writes `status` for the mod's HUD.
pub fn serve(config: &Config) -> Result<(), String> {
    let shared = Shared::new(&config.home);
    let _lock = lock(&shared)?;
    if let Some(path) = &config.channels {
        return serve_channels(&shared, path);
    }
    // Always start muted: a stale control file from a crash must never open
    // the microphone before the player asks for it.
    let _ = shared.set_transmitting(false);
    let mut wanted = false;
    let client = Client::start_with(config.server, config.key)?;
    client.set_active(true);
    client.muted.store(true, Ordering::Release);
    eprintln!(
        "Horizon voice helper: relay {}, microphone muted",
        config.server
    );
    let mut last = Status::default();
    let mut beat = 0u64;
    let mut last_write = std::time::Instant::now() - STATUS_INTERVAL;
    // The mod writes its control file in place, so a read can catch it
    // half-written. Act only when two polls agree.
    let mut pending: Option<bool> = None;
    loop {
        if let Some(error) = client.error() {
            let _ = shared.write_status(&Status {
                error,
                beat: beat.wrapping_add(1),
                ..Status::default()
            });
            return Err("voice audio stopped; restart the helper".into());
        }
        let now = shared.transmitting();
        if now != wanted {
            if pending == Some(now) {
                wanted = now;
                pending = None;
                client.muted.store(!wanted, Ordering::Release);
                eprintln!(
                    "Horizon voice helper: microphone {}",
                    if wanted { "on" } else { "muted" }
                );
            } else {
                pending = Some(now);
            }
        } else {
            pending = None;
        }
        beat = beat.wrapping_add(1);
        let status = Status {
            connected: client.connected(),
            transmitting: client.transmitting(),
            peers: client.peers(),
            error: String::new(),
            beat,
        };
        if (status != last && last_write.elapsed() >= Duration::from_millis(20))
            || last_write.elapsed() >= STATUS_INTERVAL
        {
            let _ = shared.write_status(&status);
            last = status;
            last_write = std::time::Instant::now();
        }
        thread::sleep(CONTROL_INTERVAL);
    }
}

/// No matching game connection means no audio worker, no relay and no mic.
fn serve_channels(shared: &Shared, path: &Path) -> Result<(), String> {
    use crate::channels::{self, Channel};
    let _ = shared.set_transmitting(false);
    let mut current: Option<Channel> = None;
    let mut client: Option<Client> = None;
    let mut wanted = false;
    let mut pending = None;
    let mut beat = 0u64;
    loop {
        // Reload routes every poll: admin updates require no user commands.
        let loaded = channels::load(path);
        let next = loaded
            .as_ref()
            .ok()
            .and_then(|routes| channels::select(routes, &channels::connections()).cloned());
        if next != current {
            if let Some(old) = client.take() {
                old.set_active(false);
                drop(old);
            }
            shared.set_transmitting(false).map_err(|e| e.to_string())?;
            wanted = false;
            pending = None;
            current = next;
            if let Some(channel) = &current {
                eprintln!("Voice channel: {} (microphone muted)", channel.name);
                let audio = Client::start_with(channel.relay, channel.key)?;
                audio.set_active(true);
                client = Some(audio);
            } else {
                eprintln!("Voice idle: no supported server connection");
            }
        }
        let mut status = Status {
            beat,
            ..Status::default()
        };
        if let Some(audio) = &client {
            if let Some(error) = audio.error() {
                return Err(error);
            }
            let now = shared.transmitting();
            if now != wanted && pending == Some(now) {
                wanted = now;
                pending = None;
                audio.muted.store(!wanted, Ordering::Release);
            } else if now != wanted {
                pending = Some(now);
            } else {
                pending = None;
            }
            status.connected = audio.connected();
            status.transmitting = audio.transmitting();
            status.peers = audio.peers();
        } else {
            shared.set_transmitting(false).map_err(|e| e.to_string())?;
            status.error = loaded
                .err()
                .unwrap_or_else(|| "Join a voice-enabled server".into());
        }
        shared.write_status(&status).map_err(|e| e.to_string())?;
        beat = beat.wrapping_add(1);
        thread::sleep(STATUS_INTERVAL);
    }
}

/// One helper per game folder: two would fight over the control file and open
/// the microphone twice. The returned file holds the lock until it is dropped.
fn lock(shared: &Shared) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(shared.dir())
        .map_err(|e| format!("{}: {e}", shared.dir().display()))?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(shared.dir().join("helper.lock"))
        .map_err(|e| format!("helper.lock: {e}"))?;
    file.try_lock()
        .map_err(|_| "another horizon-voice-helper is already running".to_string())?;
    Ok(file)
}

/// `helper toggle`: flips the control file for a desktop hotkey.
pub fn toggle(home: &Path) -> Result<bool, String> {
    let shared = Shared::new(home);
    let now = !shared.transmitting();
    shared
        .set_transmitting(now)
        .map_err(|e| format!("{}: {e}", shared.dir().display()))?;
    Ok(now)
}

/// `helper on|off`.
pub fn set(home: &Path, on: bool) -> Result<(), String> {
    Shared::new(home)
        .set_transmitting(on)
        .map_err(|e| e.to_string())
}

/// `helper status`: one line for scripts and the README.
pub fn describe(home: &Path) -> String {
    let shared = Shared::new(home);
    let text = shared.read_status();
    if text.is_empty() {
        return "helper: no status yet (is horizon-voice-helper running?)".into();
    }
    let mut status = Status::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "connected" => status.connected = value.trim() == "1",
            "transmitting" => status.transmitting = value.trim() == "1",
            "peers" => status.peers = value.trim().parse().unwrap_or(0),
            "error" => status.error = value.trim().to_owned(),
            _ => {}
        }
    }
    if !status.error.is_empty() {
        return format!("helper: {}", status.error);
    }
    format!(
        "helper: {}, microphone {}, {} other player(s)",
        if status.connected {
            "connected"
        } else {
            "connecting"
        },
        if status.transmitting { "on" } else { "muted" },
        status.peers
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "horizon-voice-helper-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn control_round_trip_defaults_to_muted() {
        let home = temp();
        let shared = Shared::new(&home);
        assert!(!shared.transmitting(), "a missing file must mean muted");
        shared.set_transmitting(true).unwrap();
        assert!(shared.transmitting());
        shared.set_transmitting(false).unwrap();
        assert!(!shared.transmitting());
        assert_eq!(
            shared.dir(),
            home.join("horizon-mods/.horizon-mod-data/horizon_voice")
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn malformed_control_is_treated_as_muted() {
        let home = temp();
        let shared = Shared::new(&home);
        std::fs::create_dir_all(shared.dir()).unwrap();
        std::fs::write(shared.dir().join("control"), b"transmitt").unwrap();
        assert!(!shared.transmitting());
        std::fs::write(shared.dir().join("control"), b"transmitting=1").unwrap();
        assert!(
            shared.transmitting(),
            "a line without a newline still counts"
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn status_is_what_the_mod_reads() {
        let home = temp();
        let shared = Shared::new(&home);
        shared
            .write_status(&Status {
                connected: true,
                transmitting: true,
                peers: 2,
                beat: 7,
                ..Status::default()
            })
            .unwrap();
        let text = shared.read_status();
        assert!(text.contains("connected=1"));
        assert!(text.contains("transmitting=1"));
        assert!(text.contains("peers=2"));
        assert!(describe(&home).contains("microphone on"));
        assert!(describe(&home).contains("2 other"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn errors_are_one_line_so_the_hud_stays_one_line() {
        let home = temp();
        let shared = Shared::new(&home);
        shared
            .write_status(&Status {
                error: "no microphone\ncheck it".into(),
                ..Status::default()
            })
            .unwrap();
        assert_eq!(shared.read_status().matches('\n').count(), 5);
        assert!(shared.read_status().contains("beat=0"));
        assert!(describe(&home).contains("no microphone check it"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_second_helper_is_refused() {
        let home = temp();
        let shared = Shared::new(&home);
        let first = lock(&shared).unwrap();
        assert!(lock(&shared).is_err());
        drop(first);
        assert!(lock(&shared).is_ok());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn toggle_flips_the_control_file() {
        let home = temp();
        assert!(toggle(&home).unwrap());
        assert!(Shared::new(&home).transmitting());
        assert!(!toggle(&home).unwrap());
        set(&home, true).unwrap();
        assert!(Shared::new(&home).transmitting());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
