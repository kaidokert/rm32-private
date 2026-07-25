#!/usr/bin/env python3
"""Transit-surge autopsy: engage at 20%, ZC-trace on, step to --to pct,
capture everything until the BENCH KILL line appears (or timeout), then
pull the FROZEN blackbox. One kill per boot (guard latches)."""
import argparse, struct, time
import serial

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--to", type=int, default=60)
ap.add_argument("--out", default="surge_capture")
a = ap.parse_args()

p = serial.Serial(a.port, 2_000_000, timeout=0.05)
raw = b""
try:
    t0 = time.time()
    while time.time() - t0 < 1.6:
        p.write(b"0\n"); p.flush(); time.sleep(0.1)
    # engage at 20% and let it lock (deterministic post-5f, ~4s)
    t0 = time.time()
    while time.time() - t0 < 6.0:
        p.write(b"20\n"); p.flush(); time.sleep(0.3)
    p.read(65536)
    p.write(b"i"); p.flush(); time.sleep(0.4)
    info = p.read(65536)
    m = [l for l in info.decode(errors="replace").splitlines() if l.startswith("i step=")]
    print("engage:", m[-1] if m else info[-100:])
    # climb to 50 and dwell — the sweep's kill context (ci ~217 at 50%)
    t0 = time.time()
    while time.time() - t0 < 4.0:
        p.write(b"50\n"); p.flush(); time.sleep(0.3)
    p.read(65536)
    p.write(b"Z"); p.flush(); time.sleep(0.3)
    zack = p.read(8192)
    print("Z ack:", b"zctrace ON" in zack, zack[-60:])
    print(f"== stepping 50 -> {a.to}%")
    t0 = time.time()
    killed = False
    while time.time() - t0 < 8.0:
        p.write(f"{a.to}\n".encode()); p.flush()
        chunk = p.read(65536)
        raw += chunk
        if b"BENCH KILL" in raw:
            killed = True
            break
        time.sleep(0.1)
    # small grace read, then the frozen blackbox
    time.sleep(0.3); raw += p.read(65536)
    p.write(b"b"); p.flush(); time.sleep(1.2)
    bb = p.read(300000)
    print("KILLED" if killed else "no kill (survived the step)")
finally:
    p.write(b"w"); p.flush(); time.sleep(0.2); p.close()

open(a.out + ".bin", "wb").write(raw)
open(a.out + "_bb.txt", "wb").write(bb)
# decode the zct tail
recs = []
i = 0
while i + 15 <= len(raw):
    if raw[i] == 0x5B and raw[i+1] == 0xA9:
        fl = raw[i+2]
        z, ci, w, d, tk, av = struct.unpack_from("<6H", raw, i+3)
        recs.append((fl & 7, bool(fl & 0x80), z, ci, w, d, tk, av))
        i += 15
    else:
        i += 1
print(f"{len(recs)} zct records; last 40 before end:")
for r in recs[-40:]:
    s, old, z, ci, w, d, tk, av = r
    print(f"  s{s}{'p' if old else ' '} thiszc={z:5d} ci={ci:5d} wait={w:5d} duty={d:4d} avg={av:5d}")
kill_lines = [l for l in bb.decode(errors="replace").splitlines() if l.startswith("bb ")]
print(f"\nfrozen blackbox ({len(kill_lines)} lines), last 30:")
for l in kill_lines[-30:]:
    print(" ", l)
