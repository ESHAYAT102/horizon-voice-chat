//! Private, server-wide voice transport. Use only over Tailscale: HMAC authenticates
//! packets, while the tailnet supplies encryption and access control.
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

#[cfg(feature = "client")]
pub mod channels;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "client")]
pub mod helper;

pub const SAMPLES: usize = 320; // 20 ms, 16 kHz mono signed PCM
pub const MAX_PACKET: usize = 4 + 8 + 8 + SAMPLES * 2 + 32;
const HEADER: usize = 20;
const TAG: usize = 32;
const TTL: Duration = Duration::from_secs(4);

pub fn key_from_hex(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 {
        return Err("voice key must be 64 hexadecimal characters".into());
    }
    let mut key = [0; 32];
    for (i, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let s = std::str::from_utf8(pair).map_err(|_| "invalid voice key")?;
        key[i] = u8::from_str_radix(s, 16).map_err(|_| "invalid voice key")?;
    }
    Ok(key)
}

pub fn encode(key: &[u8; 32], id: u64, seq: u64, audio: &[i16]) -> Vec<u8> {
    assert!(audio.is_empty() || audio.len() == SAMPLES);
    let mut out = Vec::with_capacity(MAX_PACKET);
    out.extend_from_slice(b"HV01");
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&seq.to_be_bytes());
    for sample in audio {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
    mac.update(&out);
    out.extend_from_slice(&mac.finalize().into_bytes());
    out
}

pub struct Packet<'a> {
    pub id: u64,
    pub seq: u64,
    pub audio: &'a [u8],
}
pub fn decode<'a>(key: &[u8; 32], bytes: &'a [u8]) -> Option<Packet<'a>> {
    if !matches!(bytes.len(), n if n == HEADER + TAG || n == MAX_PACKET) || &bytes[..4] != b"HV01" {
        return None;
    }
    let signed = bytes.len() - TAG;
    let mut mac = Hmac::<Sha256>::new_from_slice(key).ok()?;
    mac.update(&bytes[..signed]);
    mac.verify_slice(&bytes[signed..]).ok()?;
    Some(Packet {
        id: u64::from_be_bytes(bytes[4..12].try_into().ok()?),
        seq: u64::from_be_bytes(bytes[12..20].try_into().ok()?),
        audio: &bytes[HEADER..signed],
    })
}

struct Peer {
    address: SocketAddr,
    seq: u64,
    seen: Instant,
    window: Instant,
    packets: u32,
}
pub struct Room {
    peers: HashMap<u64, Peer>,
    limit: usize,
}
impl Room {
    pub fn new(limit: usize) -> Self {
        Self {
            peers: HashMap::new(),
            limit,
        }
    }
    /// Authenticate before calling. Expire peers, reject replays/address changes,
    /// and cap each sender at 75 packets/second (normal voice is 50).
    pub fn route(
        &mut self,
        packet: &Packet<'_>,
        address: SocketAddr,
        now: Instant,
    ) -> Vec<SocketAddr> {
        self.peers.retain(|_, p| now.duration_since(p.seen) < TTL);
        if let Some(p) = self.peers.get_mut(&packet.id) {
            if p.address != address || packet.seq <= p.seq {
                return vec![];
            }
            if now.duration_since(p.window) >= Duration::from_secs(1) {
                p.window = now;
                p.packets = 0;
            }
            if p.packets >= 75 {
                return vec![];
            }
            p.packets += 1;
            p.seq = packet.seq;
            p.seen = now;
        } else {
            if self.peers.len() >= self.limit {
                return vec![];
            }
            self.peers.insert(
                packet.id,
                Peer {
                    address,
                    seq: packet.seq,
                    seen: now,
                    window: now,
                    packets: 1,
                },
            );
        }
        if packet.audio.is_empty() {
            return vec![address];
        } // authenticated heartbeat acknowledgement
        self.peers
            .iter()
            .filter(|(id, _)| **id != packet.id)
            .map(|(_, p)| p.address)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authentication_and_lengths() {
        let key = [7; 32];
        let frame = encode(&key, 1, 2, &[123; SAMPLES]);
        assert_eq!(decode(&key, &frame).unwrap().audio.len(), 640);
        assert!(decode(&[8; 32], &frame).is_none());
        for n in 0..frame.len() {
            if n != HEADER + TAG {
                assert!(decode(&key, &frame[..n]).is_none());
            }
        }
        let mut tampered = frame;
        tampered[30] ^= 1;
        assert!(decode(&key, &tampered).is_none());
    }
    #[test]
    fn routing_replay_limit_and_expiry() {
        let mut room = Room::new(2);
        let now = Instant::now();
        let a = "127.0.0.1:1".parse().unwrap();
        let b = "127.0.0.1:2".parse().unwrap();
        let p = |id, seq| Packet {
            id,
            seq,
            audio: &[0; 640],
        };
        assert!(room.route(&p(1, 1), a, now).is_empty());
        assert_eq!(room.route(&p(2, 1), b, now), vec![a]);
        assert!(room.route(&p(2, 1), b, now).is_empty());
        assert!(room.route(&p(2, 2), a, now).is_empty());
        assert!(room.route(&p(3, 1), a, now).is_empty());
        assert!(room.route(&p(3, 1), a, now + TTL).is_empty());
        assert_eq!(room.peers.len(), 1);
    }
    #[test]
    fn malformed_keys() {
        assert!(key_from_hex("nope").is_err());
        assert_eq!(key_from_hex(&"ab".repeat(32)).unwrap(), [0xab; 32]);
        assert!(key_from_hex(&"💥".repeat(16)).is_err());
    }

    #[test]
    fn flood_is_bounded_and_next_window_recovers() {
        let mut room = Room::new(2);
        let now = Instant::now();
        let a = "127.0.0.1:1".parse().unwrap();
        let b = "127.0.0.1:2".parse().unwrap();
        let p = |id, seq| Packet {
            id,
            seq,
            audio: &[0; 640],
        };
        room.route(&p(2, 1), b, now);
        for seq in 1..=75 {
            assert_eq!(room.route(&p(1, seq), a, now), vec![b]);
        }
        assert!(room.route(&p(1, 76), a, now).is_empty());
        assert_eq!(
            room.route(&p(1, 77), a, now + Duration::from_secs(1)),
            vec![b]
        );
    }
}
