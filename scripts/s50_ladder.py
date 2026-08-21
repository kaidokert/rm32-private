#!/usr/bin/env python3
"""Small qualification ladder for the S50 (stock AM32) through Betaflight.

Staircase to a capped throttle with MSP_MOTOR_TELEMETRY (bidir DSHOT)
verdicts: per-rung mean rpm + invalid%, mid-rung rpm-collapse detection.
No ESC-side instrumentation assumed. Kill guards on every exit path;
BF CLI motor stream is kept alive (<2 s command cadence — the stream
stops ~5 s after the last motor command).

Usage: python s50_ladder.py [--bf COM42] [--max 1250] [--step 50]
                            [--dwell 3.0] [--poles 14]
"""

import argparse
import struct
import sys
import time

import serial

MSP_MOTOR_TELEMETRY = 139


def msp1_frame(cmd, payload=b""):
    hdr = bytes([len(payload), cmd]) + payload
    csum = 0
    for b in hdr:
        csum ^= b
    return b"$M<" + hdr + bytes([csum])


def read_msp_reply(p, want, deadline=0.5):
    buf = b""
    t0 = time.time()
    while time.time() - t0 < deadline:
        buf += p.read(512)
        i = buf.find(b"$M>")
        if i < 0 or len(buf) < i + 5:
            continue
        n = buf[i + 3]
        if len(buf) < i + 6 + n:
            continue
        if buf[i + 4] == want:
            return buf[i + 5 : i + 5 + n]
        buf = buf[i + 3 :]
    return None


def motor_telemetry(p, poles):
    p.reset_input_buffer()
    p.write(msp1_frame(MSP_MOTOR_TELEMETRY))
    payload = read_msp_reply(p, MSP_MOTOR_TELEMETRY)
    if not payload or payload[0] < 1:
        return None
    erpm, inv = struct.unpack_from("<IH", payload, 1)
    if erpm == 0x7FFFFFFF:
        return None
    return erpm, inv / 100.0  # eRPM, invalid %


class Cli:
    def __init__(self, port):
        self.p = serial.Serial(port, 115_200, timeout=0.3)

    def enter(self):
        self.p.write(b"#\n")
        time.sleep(1.0)
        self.p.read(8192)

    def cmd(self, line):
        self.p.write(line.encode() + b"\n")
        time.sleep(0.15)
        return self.p.read(4096)

    def motor(self, value):
        self.cmd(f"motor 0 {value}")

    def close_reboot(self):
        try:
            self.cmd("motor 0 1000")
            self.p.write(b"exit\n")
            time.sleep(0.3)
        finally:
            self.p.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--max", type=int, default=1250, help="top motor value")
    ap.add_argument("--min", type=int, default=1100, help="first rung (spin floor ~10%%)")
    ap.add_argument("--step", type=int, default=50)
    ap.add_argument("--dwell", type=float, default=3.0)
    ap.add_argument("--poles", type=int, default=14)
    a = ap.parse_args()

    # Telemetry rides the same port as the CLI via MSP — but MSP does not
    # work while the CLI is active, so drive the motor over CLI and poll
    # telemetry between CLI sessions? No: MSP_MOTOR_TELEMETRY works fine
    # pre-CLI; instead we drive motors via CLI and read rpm via a second
    # pass. Simplest robust scheme (validated on the L431 bench): stay in
    # CLI for motor control; poll telemetry over MSP BEFORE and AFTER,
    # and DURING rungs via interleaved CLI-exit is too disruptive.
    # => Use MSP motor control instead: MSP_SET_MOTOR (214) keeps MSP
    # alive for telemetry polling and needs re-sending <2 s like CLI.
    MSP_SET_MOTOR = 214

    def set_motor(p, value):
        # 8 motor slots, u16 LE each
        payload = struct.pack("<8H", value, *([1000] * 7))
        p.write(msp1_frame(MSP_SET_MOTOR, payload))
        time.sleep(0.05)
        p.reset_input_buffer()

    p = serial.Serial(a.bf, 115_200, timeout=0.3)
    rungs = list(range(a.min, a.max + 1, a.step))
    results = []
    fail = None
    try:
        # idle sanity
        t = motor_telemetry(p, a.poles)
        print(f"idle telemetry: {t}")
        for rung in rungs:
            pct = (rung - 1000) / 10.0
            samples = []
            t0 = time.time()
            stall_since = None
            while time.time() - t0 < a.dwell:
                set_motor(p, rung)
                t = motor_telemetry(p, a.poles)
                if t:
                    samples.append(t)
                    if t[0] < 1000:  # eRPM ~0 while commanded
                        stall_since = stall_since or time.time()
                        if time.time() - stall_since > 2.0:
                            fail = f"stall at rung {pct:.0f}%"
                            raise KeyboardInterrupt
                    else:
                        stall_since = None
                time.sleep(0.05)
            if samples:
                tail = samples[len(samples) // 2 :]  # settled half
                mean_erpm = sum(s[0] for s in tail) / len(tail)
                worst_inv = max(s[1] for s in tail)
                lo = min(s[0] for s in tail)
                collapse = lo < 0.5 * mean_erpm
                results.append((pct, mean_erpm, worst_inv, collapse, len(samples)))
                print(f"rung {pct:4.0f}%  eRPM={mean_erpm:8.0f}  "
                      f"rpm={mean_erpm / (a.poles / 2):7.0f}  "
                      f"inv%={worst_inv:5.2f}  n={len(samples)}"
                      f"{'  COLLAPSE' if collapse else ''}")
            else:
                results.append((pct, 0, 100.0, True, 0))
                print(f"rung {pct:4.0f}%  NO TELEMETRY")
    except KeyboardInterrupt:
        pass
    finally:
        # Kill guard: always stop the motor on every exit path.
        try:
            for _ in range(3):
                set_motor(p, 1000)
                time.sleep(0.1)
        except Exception:
            pass
        p.close()

    print("\n=== S50 SMALL QUAL (BF bidir DSHOT300) ===")
    monotone = all(results[i][1] < results[i + 1][1]
                   for i in range(len(results) - 1))
    inv_ok = all(r[2] < 1.0 for r in results if r[4])
    no_collapse = not any(r[3] for r in results)
    # MSP poll cycle is ~0.35 s (write + reply deadline); ~8 samples per
    # 3 s dwell is the healthy rate — gate on enough settled samples for
    # a meaningful mean, not an aspirational count.
    got_all = all(r[4] >= 5 for r in results)
    print(f"  rungs={len(results)} monotone_rpm={monotone} "
          f"inv<1%={inv_ok} no_collapse={no_collapse} samples_ok={got_all}")
    if fail:
        print(f"  !! {fail}")
    verdict = monotone and inv_ok and no_collapse and got_all and not fail
    print(f"  verdict: {'PASS' if verdict else 'CHECK'}")
    return 0 if verdict else 1


if __name__ == "__main__":
    sys.exit(main())
