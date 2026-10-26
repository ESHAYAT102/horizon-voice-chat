//! Development-only protocol bots; no microphone or Minecraft artifacts.
use horizon_client::native_voice::{CHANNEL, FRAME, HELLO, JOIN};
use horizon_protocol::{
    PROTOCOL_VERSION,
    connection::{ConnWriter, Connection},
    crypto::offline_uuid,
    packets::{
        Empty,
        configuration::{self as cfg, ClientInformation, CustomPayload},
        handshake, login,
        play::{self, Clientbound as C, Serverbound as S},
    },
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

struct Bot {
    writer: Arc<Mutex<ConnWriter>>,
    events: Receiver<Vec<u8>>,
}
impl Bot {
    fn send(&self, data: Vec<u8>) {
        self.writer
            .lock()
            .unwrap()
            .send(
                &S::CustomPayload(CustomPayload {
                    channel: CHANNEL.into(),
                    data,
                })
                .encode(),
            )
            .unwrap();
    }
    fn connect(port: u16, name: &str, subscribe: bool) -> Self {
        let mut c = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
        c.reader
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        c.send(
            &handshake::Serverbound::Intention(handshake::Intention {
                protocol: PROTOCOL_VERSION,
                host: "127.0.0.1".into(),
                port,
                intent: handshake::LOGIN,
            })
            .encode(),
        )
        .unwrap();
        c.send(
            &login::Serverbound::Hello(login::ServerHello {
                name: name.into(),
                uuid: offline_uuid(name),
            })
            .encode(),
        )
        .unwrap();
        loop {
            let (id, bytes) = c.recv().unwrap();
            match login::Clientbound::decode(id, &bytes).unwrap() {
                login::Clientbound::Compression(p) => c.set_compression(p.threshold),
                login::Clientbound::LoginFinished(_) => {
                    c.send(&login::Serverbound::LoginAcknowledged(Empty).encode())
                        .unwrap();
                    break;
                }
                other => panic!("unexpected login {other:?}"),
            }
        }
        c.send(&cfg::Serverbound::ClientInformation(ClientInformation::new(2)).encode())
            .unwrap();
        loop {
            let (id, bytes) = c.recv().unwrap();
            match cfg::Clientbound::decode(id, &bytes).unwrap() {
                cfg::Clientbound::SelectKnownPacks(p) => {
                    c.send(
                        &cfg::Serverbound::SelectKnownPacks(cfg::SelectKnownPacks {
                            packs: p.packs,
                        })
                        .encode(),
                    )
                    .unwrap();
                }
                cfg::Clientbound::FinishConfiguration(_) => {
                    c.send(&cfg::Serverbound::FinishConfiguration(Empty).encode())
                        .unwrap();
                    break;
                }
                _ => {}
            }
        }
        let (mut reader, writer) = c.split();
        let writer = Arc::new(Mutex::new(writer));
        let w = writer.clone();
        let (tx, events) = mpsc::channel();
        thread::spawn(move || {
            while let Ok((id, bytes)) = reader.recv() {
                let Ok(packet) = C::decode(id, &bytes) else {
                    continue;
                };
                let reply = match packet {
                    C::KeepAlive(p) => Some(S::KeepAlive(p)),
                    C::PlayerPosition(p) => {
                        let mut w = w.lock().unwrap();
                        w.send(
                            &S::AcceptTeleportation(play::AcceptTeleportation {
                                id: p.teleport,
                                x: p.position[0],
                                y: p.position[1],
                                z: p.position[2],
                                yaw: p.yaw,
                                pitch: p.pitch,
                            })
                            .encode(),
                        )
                        .unwrap();
                        w.send(&S::PlayerLoaded(Empty).encode()).unwrap();
                        None
                    }
                    C::ChunkBatchFinished(_) => {
                        Some(S::ChunkBatchReceived(play::ChunkBatchReceived {
                            chunks_per_tick: 8.,
                        }))
                    }
                    C::CustomPayload(p) if p.channel == CHANNEL => {
                        if p.data == HELLO && subscribe {
                            w.lock()
                                .unwrap()
                                .send(
                                    &S::CustomPayload(CustomPayload {
                                        channel: CHANNEL.into(),
                                        data: JOIN.to_vec(),
                                    })
                                    .encode(),
                                )
                                .unwrap();
                        }
                        let _ = tx.send(p.data);
                        None
                    }
                    _ => None,
                };
                if let Some(p) = reply {
                    let _ = w.lock().unwrap().send(&p.encode());
                }
            }
        });
        let bot = Self { writer, events };
        assert_eq!(
            bot.events.recv_timeout(Duration::from_secs(30)).unwrap(),
            HELLO
        );
        bot
    }
}
impl Drop for Bot {
    fn drop(&mut self) {
        self.writer.lock().unwrap().shutdown();
    }
}

fn main() {
    let port: u16 = std::env::args().nth(1).expect("port").parse().unwrap();
    let a = Bot::connect(port, "VoiceAlice", true);
    let b = Bot::connect(port, "VoiceBob", true);
    let outsider = Bot::connect(port, "VoiceOutsider", false);
    thread::sleep(Duration::from_secs(2));
    let frame = vec![2; FRAME + 1];
    outsider.send(frame.clone());
    assert!(
        a.events.recv_timeout(Duration::from_millis(300)).is_err(),
        "unsubscribed sender forwarded"
    );
    a.send(vec![2; 17]);
    assert!(
        b.events.recv_timeout(Duration::from_millis(300)).is_err(),
        "malformed frame forwarded"
    );
    a.send(frame.clone());
    let received = b.events.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(received.len(), FRAME + 5);
    assert_eq!(&received[5..], &frame[1..]);
    assert!(
        a.events.recv_timeout(Duration::from_millis(200)).is_err(),
        "sender echoed"
    );
    assert!(
        outsider
            .events
            .recv_timeout(Duration::from_millis(200))
            .is_err(),
        "unsubscribed listener received audio"
    );
    thread::sleep(Duration::from_secs(1));
    for _ in 0..100 {
        a.send(frame.clone());
    }
    let start = Instant::now();
    let mut count = 0;
    while start.elapsed() < Duration::from_secs(2) {
        if b.events.recv_timeout(Duration::from_millis(100)).is_ok() {
            count += 1;
        }
    }
    assert!(count > 0 && count <= 60, "rate cap: {count}");
    println!(
        "PASS: server announcement, subscription, audio forwarding, identity, no echo, malformed/unsubscribed rejection, rate cap ({count}/100)"
    );
}
