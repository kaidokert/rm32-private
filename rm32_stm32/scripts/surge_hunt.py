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
ap.add_argument("--ladder", action="store_true",
                help="map_sweep profile: 10..50%% at 6s dwells before the step "
                     "(reproduces the ladder-context kill the direct step lacks)")
ap.add_argument("--zearly", action="store_true",
                help="zctrace ON from engage (catch dirty-mode engages in the act)")
a = ap.parse_args()

p = serial.Serial(a.port, 2_000_000, timeout=0.05)
raw = b""
killed = False
try:
    t0 = time.time()
    while time.time() - t0 < 1.6:
        p.write(b"0\n"); p.flush(); time.sleep(0.1)

    def dwell(pct, secs):
        # drains continuously (zct stream must not overflow the OS buffer)
        # and spots a mid-dwell guard kill (the sweep3 collapse class).
        global raw, killed
        t0 = time.time()
        while time.time() - t0 < secs:
            p.write(f"{pct}\n".encode()); p.flush()
            raw += p.read(65536)
            if b"BENCH KILL" in raw:
                killed = True
                print(f"KILLED mid-dwell at {pct}%")
                return
            time.sleep(0.1)

    def engage_info():
        p.write(b"i"); p.flush(); time.sleep(0.4)
        global raw
        info = p.read(65536)
        raw += info
        m = [l for l in info.decode(errors="replace").splitlines() if l.startswith("i step=")]
        print("engage:", m[-1] if m else info[-100:])

    if a.zearly:
        p.write(b"Z"); p.flush(); time.sleep(0.3)
        zack = p.read(8192)
        print("Z ack:", b"zctrace ON" in zack)

    if a.ladder:
        # map_sweep profile: full climb with sweep-length dwells
        dwell(10, 6.0)
        engage_info()
        for pct in range(20, a.to, 10):
            if killed:
                break
            dwell(pct, 6.0)
    else:
        # engage at 20% and let it lock (deterministic post-5f, ~4s)
        dwell(20, 6.0)
        engage_info()
        # climb to 50 and dwell — the sweep's kill context (ci ~217 at 50%)
        if not killed:
            dwell(50, 4.0)

    if not killed:
        if not a.zearly:
            p.write(b"Z"); p.flush(); time.sleep(0.3)
            zack = p.read(8192)
            print("Z ack:", b"zctrace ON" in zack, zack[-60:])
        print(f"== stepping to {a.to}%")
        t0 = time.time()
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
