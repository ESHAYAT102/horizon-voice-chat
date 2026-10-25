//! Microphone/output streams are owned by a worker, never the render thread.
use crate::{MAX_PACKET, SAMPLES, decode, encode, key_from_hex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::{HashMap, VecDeque},
    net::{SocketAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};

type Playback = Arc<Mutex<HashMap<u64, VecDeque<f32>>>>;
/// How long a peer stays "in the room" after its last audio packet.
const PEER_TTL: Duration = Duration::from_secs(3);
pub struct Client {
    pub muted: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    peers: Arc<AtomicUsize>,
    error: Arc<Mutex<Option<String>>>,
}
impl Client {
    /// Reads `HORIZON_VOICE_SERVER` and `HORIZON_VOICE_KEY`.
    pub fn start() -> Result<Self, String> {
        if std::env::var_os("HORIZON_NO_AUDIO").is_some() {
            return Err("audio is disabled".into());
        }
        let endpoint: SocketAddr = std::env::var("HORIZON_VOICE_SERVER")
            .map_err(|_| "voice server not configured")?
            .parse()
            .map_err(|_| "invalid voice server address")?;
        let key = key_from_hex(
            &std::env::var("HORIZON_VOICE_KEY").map_err(|_| "voice key not configured")?,
        )?;
        Self::start_with(endpoint, key)
    }
    /// The companion helper's entry point: configuration is explicit, so a
    /// service unit does not have to export secrets to every child process.
    pub fn start_with(endpoint: SocketAddr, key: [u8; 32]) -> Result<Self, String> {
        if std::env::var_os("HORIZON_NO_AUDIO").is_some() {
            return Err("audio is disabled".into());
        }
        let this = Self {
            muted: Arc::new(AtomicBool::new(true)),
            active: Arc::new(AtomicBool::new(false)),
            alive: Arc::new(AtomicBool::new(true)),
            connected: Arc::new(AtomicBool::new(false)),
            peers: Arc::new(AtomicUsize::new(0)),
            error: Arc::new(Mutex::new(None)),
        };
        let (muted, active, alive, connected, peers, error) = (
            this.muted.clone(),
            this.active.clone(),
            this.alive.clone(),
            this.connected.clone(),
            this.peers.clone(),
            this.error.clone(),
        );
        thread::Builder::new()
            .name("Horizon voice".into())
            .spawn(move || {
                if let Err(e) = run(
                    endpoint,
                    key,
                    muted.clone(),
                    active,
                    alive,
                    connected.clone(),
                    peers,
                ) {
                    muted.store(true, Ordering::Release);
                    connected.store(false, Ordering::Release);
                    *error.lock().unwrap() = Some(e);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(this)
    }
    pub fn set_active(&self, active: bool) {
        if !active {
            self.muted.store(true, Ordering::Release);
        }
        self.active.store(active, Ordering::Release);
    }
    pub fn mute(&self) {
        self.muted.store(true, Ordering::Release);
    }
    /// Whether the microphone is live (unmuted and connected).
    pub fn transmitting(&self) -> bool {
        !self.muted.load(Ordering::Acquire) && self.connected.load(Ordering::Acquire)
    }
    /// Whether the relay acknowledged us recently.
    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    /// Other players currently sending audio.
    pub fn peers(&self) -> usize {
        self.peers.load(Ordering::Acquire)
    }
    /// The worker stopped with this error, if any.
    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
    pub fn toggle(&self) {
        if self.connected.load(Ordering::Acquire) && self.active.load(Ordering::Acquire) {
            self.muted.fetch_xor(true, Ordering::AcqRel);
        }
    }
    pub fn status(&self) -> String {
        if let Some(e) = &*self.error.lock().unwrap() {
            return format!("Voice unavailable: {e}");
        }
        if !self.connected.load(Ordering::Acquire) {
            return "Voice connecting — mic MUTED".into();
        }
        if self.muted.load(Ordering::Acquire) {
            "Voice: MUTED [V]".into()
        } else {
            "Voice: MIC ON [V to mute]".into()
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.mute();
        self.alive.store(false, Ordering::Release);
    }
}

fn input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: SyncSender<Vec<i16>>,
    muted: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let rate = config.sample_rate.0;
    let mut phase = 0u32;
    let mut frame = Vec::with_capacity(SAMPLES);
    device
        .build_input_stream(
            config,
            move |samples: &[T], _| {
                if muted.load(Ordering::Acquire) || !active.load(Ordering::Acquire) {
                    frame.clear();
                    phase = 0;
                    return;
                }
                for chunk in samples.chunks_exact(channels) {
                    let mono = chunk
                        .iter()
                        .map(|s| <f32 as cpal::Sample>::from_sample(*s))
                        .sum::<f32>()
                        / channels as f32;
                    phase += 16000;
                    while phase >= rate {
                        phase -= rate;
                        frame.push((mono.clamp(-1.0, 1.0) * 32767.0) as i16);
                        if frame.len() == SAMPLES {
                            let _ = tx.try_send(std::mem::replace(
                                &mut frame,
                                Vec::with_capacity(SAMPLES),
                            ));
                        }
                    }
                }
            },
            move |_| {
                alive.store(false, Ordering::Release);
            },
            None,
        )
        .map_err(|e| e.to_string())
}
fn output<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    playback: Playback,
    active: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let rate = config.sample_rate.0;
    let mut phase = 0u32;
    let mut sample = 0f32;
    device
        .build_output_stream(
            config,
            move |out: &mut [T], _| {
                out.fill(T::from_sample(0.0));
                if !active.load(Ordering::Acquire) {
                    sample = 0.;
                    return;
                }
                let Ok(mut queues) = playback.try_lock() else {
                    return;
                };
                for chunk in out.chunks_exact_mut(channels) {
                    phase += 16000;
                    while phase >= rate {
                        phase -= rate;
                        sample = queues
                            .values_mut()
                            .map(|q| q.pop_front().unwrap_or(0.))
                            .sum::<f32>()
                            .clamp(-1., 1.);
                    }
                    chunk.fill(T::from_sample(sample));
                }
            },
            move |_| {
                alive.store(false, Ordering::Release);
            },
            None,
        )
        .map_err(|e| e.to_string())
}
fn run(
    endpoint: SocketAddr,
    key: [u8; 32],
    muted: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    peers: Arc<AtomicUsize>,
) -> Result<(), String> {
    let socket = UdpSocket::bind(if endpoint.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .map_err(|e| e.to_string())?;
    socket.connect(endpoint).map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(5)))
        .map_err(|e| e.to_string())?;
    let host = cpal::default_host();
    let mic = host.default_input_device().ok_or("no microphone")?;
    let speaker = host.default_output_device().ok_or("no audio output")?;
    let ic = mic.default_input_config().map_err(|e| e.to_string())?;
    let oc = speaker.default_output_config().map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::sync_channel(3);
    let playback: Playback = Arc::new(Mutex::new(HashMap::new()));
    macro_rules! input {
        ($t:ty) => {
            input::<$t>(
                &mic,
                &ic.config(),
                tx,
                muted.clone(),
                active.clone(),
                alive.clone(),
            )?
        };
    }
    macro_rules! output {
        ($t:ty) => {
            output::<$t>(
                &speaker,
                &oc.config(),
                playback.clone(),
                active.clone(),
                alive.clone(),
            )?
        };
    }
    let input = match ic.sample_format() {
        cpal::SampleFormat::F32 => input!(f32),
        cpal::SampleFormat::I16 => input!(i16),
        cpal::SampleFormat::U16 => input!(u16),
        _ => return Err("unsupported microphone sample format".into()),
    };
    let output = match oc.sample_format() {
        cpal::SampleFormat::F32 => output!(f32),
        cpal::SampleFormat::I16 => output!(i16),
        cpal::SampleFormat::U16 => output!(u16),
        _ => return Err("unsupported speaker sample format".into()),
    };
    input.play().map_err(|e| e.to_string())?;
    output.play().map_err(|e| e.to_string())?;
    let id = rand::random::<u64>();
    let mut seq = 0;
    let mut heartbeat = Instant::now() - Duration::from_secs(2);
    let mut acknowledged = Instant::now() - Duration::from_secs(10);
    let mut last: HashMap<u64, u64> = HashMap::new();
    let mut heard: HashMap<u64, Instant> = HashMap::new();
    let mut buffer = [0; MAX_PACKET + 1];
    while alive.load(Ordering::Acquire) {
        if !active.load(Ordering::Acquire) {
            muted.store(true, Ordering::Release);
            connected.store(false, Ordering::Release);
            playback.lock().unwrap().clear();
            last.clear();
            heard.clear();
            peers.store(0, Ordering::Release);
            while rx.try_recv().is_ok() {}
            thread::sleep(Duration::from_millis(20));
            continue;
        }
        // Peers leave the room after three seconds without audio, so the
        // count drops even when nothing else arrives to trigger a prune.
        let now = Instant::now();
        if !heard.is_empty() {
            heard.retain(|_, at| now.duration_since(*at) < PEER_TTL);
            peers.store(heard.len(), Ordering::Release);
        }
        if heartbeat.elapsed() >= Duration::from_secs(1) {
            seq += 1;
            socket
                .send(&encode(&key, id, seq, &[]))
                .map_err(|e| e.to_string())?;
            heartbeat = Instant::now();
        }
        if acknowledged.elapsed() > Duration::from_secs(3) {
            muted.store(true, Ordering::Release);
            connected.store(false, Ordering::Release);
        }
        while let Ok(frame) = rx.try_recv() {
            if !muted.load(Ordering::Acquire) && connected.load(Ordering::Acquire) {
                seq += 1;
                let _ = socket.send(&encode(&key, id, seq, &frame));
            }
        }
        match socket.recv(&mut buffer) {
            Ok(n) => {
                let Some(p) = decode(&key, &buffer[..n]) else {
                    continue;
                };
                if p.id == id && p.audio.is_empty() {
                    if p.seq > seq || p.seq + 100 < seq {
                        continue;
                    }
                    acknowledged = Instant::now();
                    connected.store(true, Ordering::Release);
                    continue;
                }
                if p.audio.is_empty()
                    || p.id == id
                    || last.get(&p.id).is_some_and(|seq| p.seq <= *seq)
                {
                    continue;
                }
                if !last.contains_key(&p.id) && last.len() >= 32 {
                    continue;
                }
                last.insert(p.id, p.seq);
                heard.insert(p.id, Instant::now());
                let mut queues = playback.lock().unwrap();
                let queue = queues.entry(p.id).or_default();
                if queue.len() + SAMPLES > 3200 {
                    queue.clear();
                }
                queue.extend(
                    p.audio
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.),
                );
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::ConnectionRefused
                ) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("voice audio stopped; reconnect to retry".into())
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
