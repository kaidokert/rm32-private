#!/usr/bin/env python3
"""Regime re-base: N reset-clean engages (starts census) + held-duty
dwells with zctrace, measuring per-duty miss rate / f_e stability on
the real-wait timing. One capture file per dwell."""
import argparse, struct, subprocess, time
import serial

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--probe", default="0483:374f:0037002F3234510836303532")
ap.add_argument("--starts", type=int, default=4)
ap.add_argument("--dwells", default="15,20,25,30")
ap.add_argument("--dwell-secs", type=float, default=15.0)
ap.add_argument("--out", default="rebase")
a = ap.parse_args()


def reset():
    subprocess.run(
        ["probe-rs", "reset", "--chip", "STM32L431KCUx", "--probe", a.probe],
        capture_output=True)
    time.sleep(5)


def engage(p, pct, secs):
    t0 = time.time()
    buf = b""
    while time.time() - t0 < secs:
        p.write(f"{pct}\n".encode()); p.flush()
        buf += p.read(65536)
        time.sleep(0.15)
    return buf


def info(p):
    p.read(65536)
    p.write(b"i"); p.flush(); time.sleep(0.4)
    r = p.read(65536)
    for l in r.decode(errors="replace").splitlines()[::-1]:
        if l.startswith("i step="):
            return l
    return None


# --- starts census ---
locks = 0
for n in range(a.starts):
    reset()
    p = serial.Serial(a.port, 2_000_000, timeout=0.05)
    try:
        engage(p, 0, 1.5)
        engage(p, 15, 6.0)
        line = info(p)
        ok = False
        if line:
            import re
            m = re.search(r"run=(\d+).*?avg=(\d+)", line)
            if m and m.group(1) == "1" and 0 < int(m.group(2)) < 3000:
                ok = True
        locks += ok
        print(f"start {n+1}/{a.starts}: {'LOCK' if ok else 'no'}  {line}")
    finally:
        p.write(b"w"); p.flush(); p.close()
print(f"census: {locks}/{a.starts} locks at 15%")

# --- held dwells with trace ---
for pct in [int(x) for x in a.dwells.split(",")]:
    reset()
    p = serial.Serial(a.port, 2_000_000, timeout=0.05)
    raw = b""
    try:
        engage(p, 0, 1.5)
        raw += engage(p, 15, 5.0)
        p.write(b"Z"); p.flush(); time.sleep(0.2)
        raw += p.read(65536)
        raw += engage(p, pct, a.dwell_secs)
        line = info(p)
        print(f"dwell {pct}%: {line}")
    finally:
        p.write(b"w"); p.flush(); p.close()
    fn = f"{a.out}_d{pct}.bin"
    open(fn, "wb").write(raw)
    # quick miss-rate readout
    miss = tot = 0
    i = 0
    while i + 15 <= len(raw):
        if raw[i] == 0x5B and raw[i+1] == 0xA6:
            fl = raw[i+2]
            fe = struct.unpack_from("<H", raw, i+3)[0]
            if not fl & 0x80:
                tot += 1
                miss += fe == 0xFFFF
            i += 15
        elif raw[i] == 0x5B and raw[i+1] == 0xA9:
            i += 15
        else:
            i += 1
    print(f"  probe windows {tot}, no-edge {miss} ({100*miss/max(1,tot):.1f}%) -> {fn}")
