#!/usr/bin/env python3
"""Developer test with synthetic PNGs only; no third-party assets are stored here."""
from pathlib import Path
import struct
import subprocess
import tempfile
from zipfile import ZipFile
import zlib

root = Path(__file__).resolve().parent

def png(width=16, color=(123, 45, 67, 180)):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    pixels = b"".join(b"\0" + bytes(color) * width for _ in range(16))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, 16, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))

with tempfile.TemporaryDirectory(prefix="voice-icons.", dir="/tmp/opencode") as work:
    base = Path(work)
    home = base / "game"
    home.mkdir()
    jar = base / "fixture.jar"
    names = ["microphone", "microphone_off", "disconnected"]
    def archive(data):
        with ZipFile(jar, "w") as z:
            for name in names:
                z.writestr(f"assets/voicechat/textures/icons/{name}.png", data)
    def run():
        return subprocess.run(["sh", str(root / "import-icons.sh"), str(jar), str(home)],
                              capture_output=True, text=True)
    first = png()
    archive(first)
    assert run().returncode == 0
    target = home / "horizon-voice-icons"
    for name in names:
        assert (target / f"{name}.png").read_bytes() == first
    assert "All Rights Reserved" in (target / "ATTRIBUTION.txt").read_text()
    archive(png(color=(0, 1, 2, 255)))
    assert run().returncode == 0
    for name in names:
        backups = list(target.glob(f"{name}.png.before-import.*"))
        assert len(backups) == 1 and backups[0].read_bytes() == first
    before = (target / "microphone.png").read_bytes()
    archive(png(width=32))
    assert run().returncode != 0
    assert (target / "microphone.png").read_bytes() == before
print("PASS: exact PNG preservation, attribution, backups, invalid dimensions rejected without replacing icons")
