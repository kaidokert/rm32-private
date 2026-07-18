"""minz ZC-trace capture (differential climb trace, minz side).

Mirrors zctrace_capture.py: engage the closed loop, enable the MZT
record stream (Z key), climb the same 60->80 profile, capture raw,
decode 5B AA records, CSV out.

Record (LE): 5B AA | flags (bits0-2 sector, bit7 refined) |
period u16 (us, commutation-to-commutation) | estimate u16 (us) |
delay u16 (us armed) | duty u16 (CCR counts) | t10 u16 (10us anchor)
| qzc_off u16 (us from window open; FFFF = none).

Run WITHOUT the MAGPIE stream (g off) - wire budget.
"""

import argparse
import pathlib
import re
import struct
import sys
import time

import serial

REC = 17


def decode(buf):
    recs = []
    i = 0
    while i + REC <= len(buf):
        if buf[i] == 0x5B and buf[i + 1] == 0xAB:
            fs = buf[i + 2]
            (per, estb, est, dly, duty, t10, qoff) = struct.unpack_from(
                "<HHHHHHH", buf, i + 3)
            if 20 <= per <= 30000 and est <= 30000:
                recs.append(dict(sector=fs & 7, refined=bool(fs & 0x80),
                                 period_us=per, est_before_us=estb,
                                 est_us=est, delay_us=dly,
                                 duty=duty, t10=t10, qzc_off=qoff))
                i += REC
                continue
        i += 1
    return recs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--climb", nargs=2, type=int, default=[60, 80])
    ap.add_argument("--dwell", type=float, default=3.0)
    ap.add_argument("--step-wait", type=float, default=0.7)
    ap.add_argument("--tag", default="mzt")
    a = ap.parse_args()
    lo, hi = a.climb

    ser = serial.Serial("COM41", 2_000_000, timeout=0.05)
    cap = bytearray()

    def key(k, wait):
        ser.write(k.encode())
        ser.flush()
        t0 = time.monotonic()
        buf = b""
        while time.monotonic() - t0 < wait:
            b = ser.read(8192)
            buf += b
            cap.extend(b)
        return buf.decode("utf-8", "replace")

    def press_until(k, want, tries=3):
        for _ in range(tries):
            if want in key(k, 0.5):
                return True
        return False

    try:
        press_until("D", "AM32")
        press_until("M", "SWIFT")
        engaged = False
        for _ in range(8):
            key("q", 2.0)
            key("y", 3.0)
            m = re.search(r"cl: ACTIVE f_e=(\d+)Hz", key("i", 1.2))
            # junk-crawl guard (mzt_fatal4: a 29 Hz "lock" passed the
            "            # ACTIVE-only check and OC-tripped): require a sane f_e.",
            if m and int(m.group(1)) > 250:
                engaged = True
                break
            key("w", 1.0)
        if not engaged:
            raise SystemExit("no engage in 8")
        print("engaged; ZT on from engage (deaths land pre-band too)")
        # stateful toggle: confirm ON via the echo (the D/M lesson)
        for _ in range(3):
            if "zt trace = on" in key("Z", 0.4):
                break
        n0 = len(cap)
        died = False
        # climb from the engage amp (~15) to lo under trace
        for _ in range(lo - 15):
            key("a", 0.12)
        time.sleep(1.0)
        m = re.search(r"cl: ACTIVE f_e=(\d+)Hz", key("i", 1.2))
        if not m:
            died = True
            print("DIED pre-band - fatal capture")
        else:
            print(f"at {lo}: {m.group(1)}Hz; tracing band climb")
            key("", a.dwell)
            for _ in range(hi - lo):
                key("a", a.step_wait)
            # SILENT dwell: the i-poll flooded the wire and cost the
            # crisis records (mzt_fatal6 queue drops at the death) -
            # capture quietly, detect death post-hoc.
            key("", a.dwell)
        # stream OFF first, then a CLEAN liveness read (mzt_fatal8:
        "        # the i-echo interleaved with trace binary and false-",
        "        # negatived - the script killed a healthy motor).",
        for _ in range(3):
            if "zt trace = off" in key("Z", 0.4):
                break
        out = key("i", 1.2)
        died = "cl: ACTIVE" not in out
        seg = bytes(cap[n0:])
        print("outcome:", "DIED (fatal capture)" if died else "survived")
    finally:
        key("w", 0.5)
        key("w", 0.3)
        ser.close()

    if 'seg' not in dir():
        seg = bytes(cap)
    raw_path = pathlib.Path("captures") / f"{a.tag}_raw.bin"
    raw_path.write_bytes(cap)
    recs = decode(bytes(cap))
    csv_path = pathlib.Path("captures") / f"{a.tag}_trace.csv"
    with open(csv_path, "w") as f:
        f.write("sector,refined,period_us,est_before_us,est_us,delay_us,duty,t10,qzc_off\n")
        for r in recs:
            f.write(f"{r['sector']},{int(r['refined'])},{r['period_us']},"
                    f"{r['est_before_us']},{r['est_us']},{r['delay_us']},"
                    f"{r['duty']},{r['t10']},{r['qzc_off']}\n")
    print(f"trace window: {len(seg)} B -> {len(recs)} records")
    print(f"raw: {raw_path}  csv: {csv_path}")
    if recs:
        print("sample:", recs[len(recs) // 2])


if __name__ == "__main__":
    main()
