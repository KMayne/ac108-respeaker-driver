#!/usr/bin/env python3
"""Record from the HAT over SSH and look at / listen to each mic.

    tools/capture.py [seconds] [--host voice-pi] [--out DIR] [--no-open]

Records with arecord on the Pi (default 16-bit slot layout: 2 x 32-bit ALSA
channels at 16 kHz, two mics per channel), copies it here, splits it into the
4 mics, and writes:

    capture-4mic.wav     all 4 mics, one channel each
    mic1.wav .. mic4.wav one file per mic
    index.html           waveforms laid out like the board, with play buttons

then opens index.html. Standard library only.
"""

import argparse
import array
import datetime
import html
import json
import math
import os
import subprocess
import sys
import wave
import webbrowser

RATE = 16_000
DEVICE = "hw:ac1084mic"
# Looking down on the HAT with the Pi's USB/Ethernet ports to the right.
LAYOUT = {1: "near left", 2: "far left", 3: "far right", 4: "near right"}
GRID = [[2, 3], [1, 4]]  # far row, then near row


def record(host, seconds, remote="/tmp/capture.wav"):
    cmd = f"arecord -q -D {DEVICE} -c 2 -r {RATE} -f S32_LE -d {seconds} {remote}"
    print(f"recording {seconds} s on {host}...", flush=True)
    subprocess.run(["ssh", "-o", "BatchMode=yes", host, cmd], check=True)
    return remote


def fetch(host, remote, local):
    subprocess.run(["scp", "-q", f"{host}:{remote}", local], check=True)


def split(raw_path):
    """2 x 32-bit ALSA frames -> list of 4 per-mic arrays of int16 (see src/frames.rs)."""
    w = wave.open(raw_path)
    if (w.getnchannels(), w.getsampwidth()) != (2, 4):
        sys.exit(f"{raw_path}: expected 2 x 32-bit channels, got {w.getnchannels()} x {8 * w.getsampwidth()}-bit")
    a = array.array("i")
    a.frombytes(w.readframes(w.getnframes()))
    mics = [array.array("h") for _ in range(4)]
    for f in range(len(a) // 2):
        x, y = a[2 * f], a[2 * f + 1]
        for m, v in zip(mics, ((x >> 16), x, (y >> 16), y)):
            v &= 0xFFFF
            m.append(v - 0x10000 if v & 0x8000 else v)
    return mics, w.getframerate()


def write_wav(path, channels, rate):
    n = len(channels[0])
    inter = array.array("h", bytes(2 * n * len(channels)))
    for c, data in enumerate(channels):
        inter[c :: len(channels)] = data
    with wave.open(path, "wb") as o:
        o.setnchannels(len(channels))
        o.setsampwidth(2)
        o.setframerate(rate)
        o.writeframes(inter.tobytes())


def envelope(data, columns=1200):
    """Min/max per column, scaled to -1..1, for drawing."""
    step = max(1, len(data) // columns)
    out = []
    for i in range(0, len(data) - step + 1, step):
        seg = data[i : i + step]
        out.append([round(min(seg) / 32768, 4), round(max(seg) / 32768, 4)])
    return out


def level_dbfs(data):
    if not data:
        return float("-inf")
    mean = sum(data) / len(data)
    rms = math.sqrt(sum((v - mean) ** 2 for v in data) / len(data))
    return 20 * math.log10(rms / 32768) if rms else float("-inf")


PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>ReSpeaker capture</title>
<style>
  :root { --bg:#fafaf8; --fg:#1d1d1b; --muted:#6b6b66; --card:#ffffff; --line:#e4e3de; --wave:#2f6fdf; --cursor:#d9480f; }
  @media (prefers-color-scheme: dark) {
    :root { --bg:#151514; --fg:#ecebe6; --muted:#9c9b95; --card:#1f1f1d; --line:#33332f; --wave:#7aa7ff; --cursor:#ff8a50; }
  }
  * { box-sizing: border-box; }
  body { margin:0; padding:24px 16px 40px; background:var(--bg); color:var(--fg);
         font:15px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif; }
  main { max-width: 1100px; margin: 0 auto; }
  h1 { font-size: 20px; margin: 0 0 4px; }
  .sub { color: var(--muted); margin: 0 0 20px; }
  .board { display:grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .edge { color: var(--muted); font-size: 13px; text-align: center; grid-column: 1 / -1; }
  .card { background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 12px; min-width: 0; }
  .head { display:flex; align-items: baseline; justify-content: space-between; gap: 8px; margin-bottom: 8px; }
  .head b { font-size: 15px; }
  .head span { color: var(--muted); font-size: 13px; font-variant-numeric: tabular-nums; }
  canvas { width: 100%; height: 120px; display: block; cursor: pointer; }
  button { margin-top: 8px; font: inherit; padding: 4px 12px; border-radius: 6px;
           border: 1px solid var(--line); background: var(--bg); color: var(--fg); cursor: pointer; }
  .all { margin: 16px 0 0; }
  @media (max-width: 640px) { .board { grid-template-columns: 1fr; } }
</style>
</head>
<body>
<main>
  <h1>ReSpeaker 4-Mic capture</h1>
  <p class="sub">__WHEN__ &middot; __SECONDS__ s &middot; 16 kHz &middot; laid out as seen from above, Pi&rsquo;s USB ports to the right. Click a waveform to play from there.</p>
  <div class="board">
    <div class="edge">far edge</div>
    __CARDS__
    <div class="edge">near edge</div>
  </div>
  <div class="all"><button id="all">Play all 4 mixed</button> <audio id="mix" src="capture-4mic.wav"></audio></div>
</main>
<script>
const data = __DATA__;
const players = {};
function css(name) { return getComputedStyle(document.documentElement).getPropertyValue(name).trim(); }
function draw(mic, pos) {
  const c = document.getElementById("c" + mic), env = data[mic];
  const dpr = window.devicePixelRatio || 1, w = c.clientWidth, h = c.clientHeight;
  c.width = w * dpr; c.height = h * dpr;
  const g = c.getContext("2d"); g.scale(dpr, dpr);
  g.fillStyle = css("--wave");
  for (let x = 0; x < w; x++) {
    const [lo, hi] = env[Math.floor(x / w * env.length)];
    const y0 = h / 2 - hi * h / 2, y1 = h / 2 - lo * h / 2;
    g.fillRect(x, y0, 1, Math.max(1, y1 - y0));
  }
  if (pos !== undefined) { g.fillStyle = css("--cursor"); g.fillRect(pos * w, 0, 2, h); }
}
function redraw() { for (const m of Object.keys(data)) draw(m, players[m] && !players[m].paused ? players[m].currentTime / players[m].duration : undefined); }
for (const m of Object.keys(data)) {
  const a = new Audio("mic" + m + ".wav"); players[m] = a;
  const btn = document.getElementById("b" + m), c = document.getElementById("c" + m);
  btn.onclick = () => { if (a.paused) { a.play(); btn.textContent = "Pause"; } else { a.pause(); btn.textContent = "Play mic " + m; } };
  a.onended = () => { btn.textContent = "Play mic " + m; redraw(); };
  c.onclick = (e) => { const r = c.getBoundingClientRect(); a.currentTime = (e.clientX - r.left) / r.width * a.duration; a.play(); btn.textContent = "Pause"; };
}
const mix = document.getElementById("mix"), allBtn = document.getElementById("all");
allBtn.onclick = () => { if (mix.paused) { mix.play(); allBtn.textContent = "Pause"; } else { mix.pause(); allBtn.textContent = "Play all 4 mixed"; } };
mix.onended = () => { allBtn.textContent = "Play all 4 mixed"; };
(function tick() { redraw(); requestAnimationFrame(tick); })();
window.addEventListener("resize", redraw);
</script>
</body>
</html>
"""


def card(mic, level):
    return (
        f'<div class="card"><div class="head"><b>Mic {mic} &middot; {html.escape(LAYOUT[mic])}</b>'
        f"<span>{level:.1f} dBFS</span></div>"
        f'<canvas id="c{mic}"></canvas><button id="b{mic}">Play mic {mic}</button></div>'
    )


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("seconds", nargs="?", type=int, default=10)
    p.add_argument("--host", default="voice-pi")
    p.add_argument("--out", help="output directory (default: captures/<timestamp>)")
    p.add_argument("--from-file", help="use this raw 2 x 32-bit capture instead of recording")
    p.add_argument("--no-open", action="store_true")
    args = p.parse_args()

    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = args.out or os.path.join("captures", stamp)
    os.makedirs(out, exist_ok=True)
    raw = os.path.join(out, "raw.wav")
    if args.from_file:
        raw = args.from_file
    else:
        fetch(args.host, record(args.host, args.seconds), raw)

    mics, rate = split(raw)
    write_wav(os.path.join(out, "capture-4mic.wav"), mics, rate)
    for i, m in enumerate(mics, 1):
        write_wav(os.path.join(out, f"mic{i}.wav"), [m], rate)

    levels = {i: level_dbfs(m) for i, m in enumerate(mics, 1)}
    cards = "\n    ".join(card(m, levels[m]) for row in GRID for m in row)
    page = (
        PAGE.replace("__CARDS__", cards)
        .replace("__DATA__", json.dumps({i: envelope(m) for i, m in enumerate(mics, 1)}))
        .replace("__WHEN__", stamp)
        .replace("__SECONDS__", f"{len(mics[0]) / rate:.1f}")
    )
    index = os.path.join(out, "index.html")
    with open(index, "w") as f:
        f.write(page)
    print(f"wrote {out}/: capture-4mic.wav, mic1-4.wav, index.html")
    for i in range(1, 5):
        print(f"  mic{i} ({LAYOUT[i]}): {levels[i]:.1f} dBFS")
    if not args.no_open:
        webbrowser.open("file://" + os.path.abspath(index))


if __name__ == "__main__":
    main()
