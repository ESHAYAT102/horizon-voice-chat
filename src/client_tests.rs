use super::*;
fn state() -> Client {
    Client {
        muted: Arc::new(AtomicBool::new(true)),
        active: Arc::new(AtomicBool::new(false)),
        alive: Arc::new(AtomicBool::new(true)),
        connected: Arc::new(AtomicBool::new(false)),
        error: Arc::new(Mutex::new(None)),
    }
}
#[test]
fn toggle_requires_join_and_relay_and_never_unmutes_on_rejoin() {
    let c = state();
    c.toggle();
    assert!(c.muted.load(Ordering::Acquire));
    c.set_active(true);
    c.toggle();
    assert!(c.muted.load(Ordering::Acquire));
    c.connected.store(true, Ordering::Release);
    c.toggle();
    assert!(!c.muted.load(Ordering::Acquire));
    assert!(c.status().contains("MIC ON"));
    c.set_active(false);
    c.set_active(true);
    assert!(c.muted.load(Ordering::Acquire));
    c.toggle();
    c.mute();
    assert!(c.muted.load(Ordering::Acquire));
    assert!(c.status().contains("MUTED"));
}
#[test]
fn dropping_client_stops_capture_and_worker() {
    let c = state();
    let alive = c.alive.clone();
    let muted = c.muted.clone();
    muted.store(false, Ordering::Release);
    drop(c);
    assert!(!alive.load(Ordering::Acquire));
    assert!(muted.load(Ordering::Acquire));
}
