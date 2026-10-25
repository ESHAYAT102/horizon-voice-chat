use horizon_voice::{MAX_PACKET, SAMPLES, decode, encode};
use std::{
    net::UdpSocket,
    process::{Child, Command, Stdio},
    time::Duration,
};
struct Relay(Child);
impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn refuses_public_bind_and_help_needs_no_secrets() {
    let result = Command::new(env!("CARGO_BIN_EXE_horizon-voice-relay"))
        .env("HORIZON_VOICE_BIND", "0.0.0.0:0")
        .env("HORIZON_VOICE_KEY", "07".repeat(32))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Tailscale"));
    let help = Command::new(env!("CARGO_BIN_EXE_horizon-voice-relay"))
        .env_remove("HORIZON_VOICE_BIND")
        .env_remove("HORIZON_VOICE_KEY")
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("HORIZON_VOICE_BIND"));
}

#[test]
fn two_clients_heartbeat_audio_no_echo_bad_key_and_replay() {
    let reservation = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let _relay = Relay(
        Command::new(env!("CARGO_BIN_EXE_horizon-voice-relay"))
            .env("HORIZON_VOICE_BIND", address.to_string())
            .env("HORIZON_VOICE_KEY", "07".repeat(32))
            .env("HORIZON_VOICE_LIMIT", "2")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let a = UdpSocket::bind("127.0.0.1:0").unwrap();
    let b = UdpSocket::bind("127.0.0.1:0").unwrap();
    for s in [&a, &b] {
        s.connect(address).unwrap();
        s.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
    }
    let key = [7; 32];
    let mut buffer = [0; MAX_PACKET + 1];
    // Bounded readiness retry, not a timing-dependent fixed startup sleep.
    let mut ready = false;
    for seq in 1..=100 {
        a.send(&encode(&key, 1, seq, &[])).unwrap();
        if let Ok(n) = a.recv(&mut buffer) {
            assert!(decode(&key, &buffer[..n]).unwrap().audio.is_empty());
            ready = true;
            break;
        }
        // UDP ECONNREFUSED can return immediately before the child is scheduled.
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(ready, "relay did not become ready");
    b.send(&encode(&key, 2, 1, &[])).unwrap();
    let n = b.recv(&mut buffer).unwrap();
    assert!(decode(&key, &buffer[..n]).unwrap().audio.is_empty());
    let frame = encode(&key, 1, 1000, &[123; SAMPLES]);
    a.send(&frame).unwrap();
    let n = b.recv(&mut buffer).unwrap();
    assert_eq!(&buffer[..n], frame.as_slice());
    assert!(a.recv(&mut buffer).is_err(), "no audio echo to sender");
    a.send(&frame).unwrap();
    assert!(b.recv(&mut buffer).is_err(), "reject replay");
    a.send(&encode(&[8; 32], 1, 1001, &[0; SAMPLES])).unwrap();
    assert!(b.recv(&mut buffer).is_err(), "reject wrong key");
    b.send(&encode(&key, 2, 2, &[-99; SAMPLES])).unwrap();
    let n = a.recv(&mut buffer).unwrap();
    assert_eq!(decode(&key, &buffer[..n]).unwrap().id, 2);
}
