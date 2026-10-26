//! Developer-only validation using the same PNG decoder as the HUD.
fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("icons folder"));
    for name in ["microphone", "microphone_off", "disconnected"] {
        let (w, h, pixels) = horizon_client::hud::decode(&root.join(format!("{name}.png"))).unwrap();
        assert_eq!((w, h, pixels.len()), (16, 16, 1024));
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
        println!("PASS: {name}: 16x16 RGBA, transparent and opaque pixels preserved");
    }
}
