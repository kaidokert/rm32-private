#!/usr/bin/env python3
"""Engage forensics: run engage attempts with the ZC trace on, classify
each as LOCK/fail, and dump per-commutation records + the blackbox for
the first N of each class. Kill guard on every exit."""
import argparse, re, struct, time
import serial

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--pct", type=int, default=20)
ap.add_argument("--want", type=int, default=2, help="captures per class")
ap.add_argument("--max-attempts", type=int, default=12)
a = ap.parse_args()
IRE = re.compile(rb"i step=\d+ old=(\d+) run=(\d+) ci=\d+ avg=(\d+) ")

def decode_zct(raw):
    out, i = [], 0
    while i + 15 <= len(raw):
        if raw[i] == 0x5B and raw[i+1] == 0xA9:
            fl = raw[i+2]
            thiszc, ci, wait, duty, tenkhz, avg = struct.unpack_from("<6H", raw, i+3)
            out.append(dict(step=fl & 7, old=bool(fl & 0x80),
                            thiszc=thiszc, ci=ci, wait=wait, duty=duty, avg=avg))
            i += 15
        else:
            i += 1
    return out

p = serial.Serial(a.port, 2_000_000, timeout=0.05)
got = {"LOCK": 0, "fail": 0}
try:
    for att in range(a.max_attempts):
        if got["LOCK"] >= a.want and got["fail"] >= a.want:
            break
        t0 = time.time()
        while time.time() - t0 < 1.6:
            p.write(b"0\n"); p.flush(); time.sleep(0.1)
        p.read(65536)
        p.write(b"Z"); p.flush(); time.sleep(0.1); p.read(4096)
        raw = b""
        t0 = time.time()
        while time.time() - t0 < 3.0:
            p.write(f"{a.pct}\n".encode()); p.flush()
            raw += p.read(65536); time.sleep(0.15)
        p.write(b"Z"); p.flush(); time.sleep(0.2); raw += p.read(65536)
        p.write(b"i"); p.flush(); time.sleep(0.4)
        m = None
        for m in IRE.finditer(p.read(65536)): pass
        cls = "?"
        if m:
            old, run, avg = int(m.group(1)), int(m.group(2)), int(m.group(3))
            fe = 2e6/(6*avg) if avg else 0
            cls = "LOCK" if (run and not old and fe > 400) else "fail"
        recs = decode_zct(raw)
        keep = cls in got and got.get(cls, 99) < a.want
        print(f"attempt {att+1}: {cls} f_e={fe:.0f} zct_records={len(recs)}"
              + ("  [captured]" if keep else ""))
        if keep:
            got[cls] += 1
            tag = f"{cls}{got[cls]}"
            # last 30 records: the engage tail
            for r in recs[-30:]:
                print(f"  {tag} s{r['step']}{'p' if r['old'] else ' '} "
                      f"thiszc={r['thiszc']:5d} ci={r['ci']:5d} wait={r['wait']:5d} "
                      f"duty={r['duty']:4d} avg={r['avg']:5d}")
            p.write(b"b"); p.flush()
            time.sleep(1.0)
            bb = p.read(200000).decode(errors="replace")
            lines = [l for l in bb.splitlines() if l.startswith("bb ")]
            for l in lines[-12:]:
                print(f"  {tag} {l}")
        p.write(b"s\n"); p.flush(); time.sleep(1.2)
finally:
    p.write(b"w"); p.flush(); time.sleep(0.2); p.close()
print("done:", got)
