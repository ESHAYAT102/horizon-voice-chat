//! Linux connection-based channel selection, without modifying Horizon.
//! Only established sockets owned by a non-dedicated horizon-client count.
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub name: String,
    pub game: SocketAddr,
    pub relay: SocketAddr,
    pub key: [u8; 32],
}

/// One route per line: name game-address relay-address key (no shell evaluation).
pub fn load(path: &Path) -> Result<Vec<Channel>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("channels: {e}"))?;
    parse(&text)
}

fn parse(text: &str) -> Result<Vec<Channel>, String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 4 {
            return Err(format!(
                "channels line {}: expected name game-address relay-address key",
                i + 1
            ));
        }
        let game = fields[1]
            .parse()
            .map_err(|_| format!("channels line {}: invalid game address", i + 1))?;
        let relay: SocketAddr = fields[2]
            .parse()
            .map_err(|_| format!("channels line {}: invalid relay address", i + 1))?;
        let safe = match relay.ip() {
            IpAddr::V4(ip) => {
                ip.is_loopback() || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
            }
            IpAddr::V6(ip) => ip.is_loopback(),
        };
        if !safe {
            return Err("channel relay must use Tailscale or loopback".into());
        }
        if !seen.insert(game) {
            return Err(format!("channels line {}: duplicate game address", i + 1));
        }
        out.push(Channel {
            name: fields[0].into(),
            game,
            relay,
            key: crate::key_from_hex(fields[3])?,
        });
    }
    Ok(out)
}

fn address(text: &str, v6: bool) -> Option<SocketAddr> {
    let (ip, port) = text.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let ip = if v6 {
        if ip.len() != 32 {
            return None;
        }
        let mut bytes = [0; 16];
        for (i, chunk) in bytes.chunks_mut(4).enumerate() {
            chunk.copy_from_slice(
                &u32::from_str_radix(&ip[i * 8..i * 8 + 8], 16)
                    .ok()?
                    .to_ne_bytes(),
            );
        }
        let ip = Ipv6Addr::from(bytes);
        ip.to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ip))
    } else {
        IpAddr::V4(Ipv4Addr::from(
            u32::from_str_radix(ip, 16).ok()?.to_ne_bytes(),
        ))
    };
    Some(SocketAddr::new(ip, port))
}

fn sockets(text: &str, v6: bool, owned: &HashSet<String>) -> Vec<SocketAddr> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<_> = line.split_whitespace().collect();
            if *f.get(3)? != "01" || !owned.contains(*f.get(9)?) {
                return None;
            }
            address(f[2], v6)
        })
        .collect()
}

/// Read-only /proc inspection; permission failures fail closed (no voice).
pub fn connections() -> Vec<SocketAddr> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let p = entry.path();
        let Ok(exe) = std::fs::read_link(p.join("exe")) else {
            continue;
        };
        if exe.file_name().is_none_or(|n| n != "horizon-client") {
            continue;
        }
        let Ok(cmd) = std::fs::read(p.join("cmdline")) else {
            continue;
        };
        if cmd.split(|b| *b == 0).any(|s| s == b"--dedicated") {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(p.join("fd")) else {
            continue;
        };
        let owned: HashSet<String> = fds
            .flatten()
            .filter_map(|fd| {
                let link = std::fs::read_link(fd.path()).ok()?;
                let s = link.to_str()?;
                Some(s.strip_prefix("socket:[")?.strip_suffix(']')?.to_owned())
            })
            .collect();
        for (file, v6) in [("tcp", false), ("tcp6", true)] {
            if let Ok(text) = std::fs::read_to_string(p.join("net").join(file)) {
                out.extend(sockets(&text, v6, &owned));
            }
        }
    }
    out
}

/// Multiple game sessions are ambiguous: do not leak one session into another.
pub fn select<'a>(channels: &'a [Channel], connections: &[SocketAddr]) -> Option<&'a Channel> {
    let games: HashSet<_> = connections.iter().copied().collect();
    let mut found = channels.iter().filter(|c| games.contains(&c.game));
    let first = found.next()?;
    if found.any(|c| c.name != first.name || c.relay != first.relay || c.key != first.key) {
        None
    } else {
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn routes() -> Vec<Channel> {
        parse(&format!("Cosmos 127.0.0.1:25565 100.104.12.117:24454 {}\nHomelab 100.67.77.122:25565 100.67.77.122:24455 {}", "ab".repeat(32), "cd".repeat(32))).unwrap()
    }
    #[test]
    fn follows_server_and_disconnects() {
        let c = routes();
        assert_eq!(select(&c, &[c[0].game]).unwrap().name, "Cosmos");
        assert_eq!(select(&c, &[c[1].game]).unwrap().name, "Homelab");
        assert!(select(&c, &[]).is_none());
        assert!(select(&c, &[c[0].game, c[1].game]).is_none());
        assert!(select(&c, &["1.2.3.4:25565".parse().unwrap()]).is_none());
    }
    #[test]
    fn only_owned_established_sockets() {
        let owned = HashSet::from(["123".into()]);
        let text = "header\n0: 00000000:0000 0100007F:63DD 01 0 0 0 1000 0 123\n0: 00000000:0000 0100007F:63DD 01 0 0 0 1000 0 456\n0: 00000000:0000 0100007F:63DD 06 0 0 0 1000 0 123\n";
        assert_eq!(
            sockets(text, false, &owned),
            vec!["127.0.0.1:25565".parse::<SocketAddr>().unwrap()]
        );
        assert_eq!(
            address("0000000000000000FFFF00000100007F:63DD", true),
            Some("127.0.0.1:25565".parse().unwrap())
        );
    }
    #[test]
    fn rejects_bad_and_public_routes() {
        assert!(parse("oops").is_err());
        assert!(parse(&format!("x 127.0.0.1:1 8.8.8.8:2 {}", "ab".repeat(32))).is_err());
    }
}
