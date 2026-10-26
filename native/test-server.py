#!/usr/bin/env python3
"""Developer-only test: scratch dedicated server plus protocol bots, never real saves."""
import argparse
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time

p = argparse.ArgumentParser()
p.add_argument("--game", type=Path, required=True)
p.add_argument("--probe", type=Path, required=True)
p.add_argument("--resources", type=Path, required=True)
p.add_argument("--accept-eula", action="store_true")
args = p.parse_args()
if not args.accept_eula:
    p.error("Testing the game requires accepting Minecraft's EULA (--accept-eula).")
base = Path(tempfile.mkdtemp(prefix="native-voice-e2e.", dir="/tmp/opencode"))
env = dict(os.environ)
for key, name in [("HORIZON_HOME", "game"), ("HORIZON_LAUNCHER_HOME", "launcher"),
                  ("XDG_DATA_HOME", "data"), ("XDG_CONFIG_HOME", "config")]:
    folder = base / name
    folder.mkdir()
    env[key] = str(folder)
env.update(HORIZON_NO_AUDIO="1", HORIZON_LAUNCHER_NO_SELF_UPDATE="1",
           HORIZON_NATIVE_VOICE="1")
home = base / "game"
(home / "eula.txt").write_text("eula=true\n")
with socket.socket() as s:
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
(home / "server.properties").write_text(
    f"server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\n"
    "white-list=false\nmax-players=4\nview-distance=2\nsimulation-distance=2\n"
    "level-name=VoiceTest\nlevel-seed=123\ngamemode=creative\nspawn-protection=0\n"
)
log_path = base / "server.log"
with log_path.open("w") as log:
    server = subprocess.Popen(
        [str(args.game.resolve()), "--resources", str(args.resources.resolve()),
         "--dedicated", "--nogui"], cwd=home, env=env, stdout=log,
        stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, start_new_session=True,
    )
    try:
        deadline = time.monotonic() + 180
        while time.monotonic() < deadline:
            if server.poll() is not None:
                raise RuntimeError(f"server stopped; see {log_path}")
            text = log_path.read_text(errors="replace")
            if 'Done (' in text or 'For help, type' in text:
                break
            time.sleep(0.2)
        else:
            raise RuntimeError(f"server readiness timed out; see {log_path}")
        subprocess.run([str(args.probe.resolve()), str(port)], env=env, check=True, timeout=100)
    finally:
        os.killpg(server.pid, signal.SIGTERM)
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(server.pid, signal.SIGKILL)
            server.wait()
print(f"Scratch test artifacts: {base}")
