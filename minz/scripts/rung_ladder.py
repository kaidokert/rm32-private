"""Rung-stepped ladder: climb WITHOUT the ZT stream (clean duplex,
echo-verified amp), capture WITH it at each dwell rung.

Full-duplex saturation discovery (mzt_90b..i): with ZT streaming at
speed, host->device keys stop arriving entirely (usart2/s: 0) - the
band climb silently capped at amp 60 while runs reported COMPLETED.
Same physics as AM32's own <=80%% trace assert; their UART throttle
survives because their keepalives are sparse half-duplex traffic.

Per rung: Z-off -> echo-verified climb -> Z-on -> dwell capture ->
liveness by record flow. Per-rung CSV + immediate post-mortem hook.

Usage: python scripts/rung_ladder.py --rungs 60,65,70,75,80,85,90 --dwell 8 --tag rl1
"""

import argparse
import pathlib
import re
import struct
import sys
import time

import serial

REC = 21


def decode(buf):
    recs = []
    i = 0
    while i + REC <= len(buf):
        if buf[i] == 0x5B and buf[i + 1] == 0xAC:
            fs = buf[i + 2]
            v = struct.unpack_from("<HHHHHHHHH", buf, i + 3)
            per, estb, est, dly, duty, t10, qoff, raw_iv, stiff = v
            if 20 <= per <= 30000 and est <= 30000:
                recs.append(dict(sector=fs & 7, refined=bool(fs & 0x80),
                                 period_us=per, est_before_us=estb, est_us=est,
                                 delay_us=dly, duty=duty, t10=t10,
                                 qzc_off=qoff, raw_iv_us=raw_iv, stiff_us=stiff))
                i += REC
                continue
        i += 1
    return recs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rungs", default="60,65,70,75,80,85,90")
    ap.add_argument("--dwell", type=float, default=8.0)
    ap.add_argument("--tag", default="rl")
    a = ap.parse_args()
    rungs = [int(x) for x in a.rungs.split(",")]

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

    def press_until(k, want, tries=7):
        for _ in range(tries):
            if want in key(k, 0.5):
                return True
        return False

    def zt(on):
        want = "zt trace = on" if on else "zt trace = off"
        for _ in range(4):
            if want in key("Z", 0.4):
                return True
        return False

    def climb_to(target):
        cur = None
        stuck = 0
        for _ in range(200):
            out = key("a", 0.15)
            m = re.findall(r"amp=(\d+)", out)
            if m:
                new = int(m[-1])
                stuck = 0 if new != cur else stuck + 1
                cur = new
                if cur >= target:
                    return cur
            else:
                stuck += 1
            if stuck > 25:
                return cur
        return cur

    results = []
    try:
        key("w", 0.5)
        press_until("D", "AM32")
        press_until("M", "SWIFT")
        engaged = False
        for att in range(8):
            key("Y", 5.0)
            m = re.search(r"cl: ACTIVE f_e=(\d+)Hz", key("i", 1.2))
            if m and int(m.group(1)) > 100:
                engaged = True
                break
            key("w", 0.5)
            time.sleep(2.5)
        if not engaged:
            print("NOENGAGE")
            sys.exit(1)
        for rung in rungs:
            reached = climb_to(rung)
            if reached is None or reached < rung:
                print(f"rung {rung}: climb stalled at {reached}")
                results.append((rung, "CLIMB-STALL", 0))
                break
            time.sleep(1.0)   # slew settle
            zt(True)
            n0 = len(cap)
            key("", a.dwell)
            zt(False)
            seg = bytes(cap[n0:])
            recs = decode(seg)
            # liveness: record flow at the dwell tail
            alive = len(recs) > 50
            per = [r["period_us"] for r in recs if 0 < r["duty"] <= 3332]
            hz = 1e6 / (6 * min(per)) if per else 0
            print(f"rung {rung}: reached {reached}, {len(recs)} recs, "
                  f"peak {hz:.0f}Hz, {'ALIVE' if alive else 'DEAD'}")
            path = pathlib.Path("captures") / f"{a.tag}_rung{rung}.csv"
            with open(path, "w") as f:
                f.write("sector,refined,period_us,est_before_us,est_us,"
                        "delay_us,duty,t10,qzc_off,raw_iv_us,stiff_us\n")
                for r in recs:
                    f.write(f"{r['sector']},{int(r['refined'])},{r['period_us']},"
                            f"{r['est_before_us']},{r['est_us']},{r['delay_us']},"
                            f"{r['duty']},{r['t10']},{r['qzc_off']},"
                            f"{r['raw_iv_us']},{r['stiff_us']}\n")
            results.append((rung, "ALIVE" if alive else "DEAD", len(recs)))
            if not alive:
                print("death at this rung - stopping ladder for post-mortem")
                break
    finally:
        key("w", 0.5)
        key("w", 0.3)
        ser.close()
        pathlib.Path("captures") / f"{a.tag}_full_raw.bin"
        (pathlib.Path("captures") / f"{a.tag}_full_raw.bin").write_bytes(bytes(cap))
    print("LADDER:", results)


if __name__ == "__main__":
    main()
