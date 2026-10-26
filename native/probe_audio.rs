//! Developer-only device smoke test; stays muted and sends nothing.
fn main() {
    let audio = horizon_client::native_voice::audio::Audio::start().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(audio.muted());
    assert!(audio.drain().is_empty());
    assert_eq!(audio.error(), None);
    drop(audio);
    println!("PASS: microphone/playback worker starts muted, sends no audio, and drops cleanly");
}
