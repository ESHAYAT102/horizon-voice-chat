use horizon_voice::{MAX_PACKET, Room, decode, key_from_hex};
use std::{net::UdpSocket, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [] => {}
        [flag] if flag == "--help" || flag == "-h" => {
            println!(
                "Horizon voice relay\n\nUsage: horizon-voice-relay\n\nEnvironment:\n  HORIZON_VOICE_BIND   Tailscale IPv4 address:UDP port\n  HORIZON_VOICE_KEY    Shared 64-character hexadecimal room key\n  HORIZON_VOICE_LIMIT  Maximum clients (default: 2, range: 1..32)\n\nUse Tailscale: voice packets are authenticated, not independently encrypted."
            );
            return Ok(());
        }
        [flag] if flag == "--version" || flag == "-V" => {
            println!("horizon-voice-relay {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => return Err("unknown argument; use --help".into()),
    }
    let address: std::net::SocketAddr = std::env::var("HORIZON_VOICE_BIND")
        .map_err(|_| "set HORIZON_VOICE_BIND to the relay's Tailscale IPv4 address:port")?
        .parse()?;
    // Do not accidentally expose unencrypted voice transport on a public interface.
    let private = match address.ip() {
        std::net::IpAddr::V4(ip) => {
            ip.is_loopback() || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
        }
        std::net::IpAddr::V6(ip) => ip.is_loopback(),
    };
    if !private {
        return Err(
            "bind voice relay to its Tailscale IPv4 address (or loopback for tests)".into(),
        );
    }
    let key = key_from_hex(
        &std::env::var("HORIZON_VOICE_KEY")
            .map_err(|_| "set HORIZON_VOICE_KEY to a random 64-character hexadecimal key")?,
    )?;
    let limit: usize = std::env::var("HORIZON_VOICE_LIMIT")
        .unwrap_or("2".into())
        .parse()?;
    if !(1..=32).contains(&limit) {
        return Err("voice limit must be 1..32".into());
    }
    let socket = UdpSocket::bind(address)?;
    let mut room = Room::new(limit);
    let mut buffer = [0; MAX_PACKET + 1];
    eprintln!("Horizon voice relay listening on {address}, maximum {limit} clients");
    loop {
        let (n, source) = socket.recv_from(&mut buffer)?;
        let Some(packet) = decode(&key, &buffer[..n]) else {
            continue;
        };
        for target in room.route(&packet, source, Instant::now()) {
            let _ = socket.send_to(&buffer[..n], target);
        }
    }
}
