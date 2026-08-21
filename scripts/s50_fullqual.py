#!/usr/bin/env python3
"""Full qualification sweep for the S50 (stock AM32) via BF MSP.

Phases:
  A. slow ladder 10->100->10 (2% rungs, settled-mean verdicts)
  B. fast ramp 10->100->10 (continuous, trajectory sanity)
  C. step throttle 20<->100 cycles (slam recovery)

Instruments: MSP_SET_MOTOR drive + MSP_MOTOR_TELEMETRY (bidir DSHOT
rpm + invalid%, ~3 Hz poll ceiling). No voltage/current channel on
this rig — 100% dwell kept short. Kill guards on every exit path.

Usage: python s50_fullqual.py [--bf COM42] [--poles 14]
"""

import argparse
import struct
import sys
import threading
import time

import serial

from s50_ladder import msp1_frame, motor_telemetry


class Kiss(threading.Thread):
    """Background KISS reader: latest (temp, volt, curr) + min-volt and
    max-curr high-water marks. Sag guard: volt < floor sustained trips."""

    def __init__(self, port, volt_floor=9.6, floor_hold_s=1.0):
        super().__init__(daemon=True)
        self.p = serial.Serial(port, 115_200, timeout=0.2)
        self.latest = None
        self.min_v = 999.0
        self.max_a = 0.0
        self.sag_tripped = False
        self._floor = volt_floor
        self._hold = floor_hold_s
        self._low_since = None
        self._win = []
        self._low = 0
        self._stop = False

    @staticmethod
    def crc8(data):
        crc = 0
        for b in data:
            crc ^= b
            for _ in range(8):
                crc = ((crc << 1) ^ 0x07) & 0xFF if crc & 0x80 else (crc << 1) & 0xFF
        return crc

    def run(self):
        buf = b""
        while not self._stop:
            buf += self.p.read(256)
            while len(buf) >= 10:
                frame, rest = buf[:10], buf[10:]
                if self.crc8(frame[:9]) != frame[9]:
                    buf = buf[1:]
                    continue
                buf = rest
                temp = frame[0]
                volt = ((frame[1] << 8) | frame[2]) / 100.0
                curr = ((frame[3] << 8) | frame[4]) / 100.0
                self.latest = (temp, volt, curr)
                # ~11% of frames corrupt under motor load; CRC8 lets
                # ~1/256 of those through — median-of-5 window + range
                # gates keep poisoned frames out of marks and the guard.
                if 3.0 < volt < 30.0 and curr < 60.0:
                    self._win.append((volt, curr))
                    if len(self._win) > 5:
                        self._win.pop(0)
                    if len(self._win) == 5:
                        mv = sorted(v for v, _ in self._win)[2]
                        mc = sorted(c for _, c in self._win)[2]
                        self.min_v = min(self.min_v, mv)
                        self.max_a = max(self.max_a, mc)
                        if mv < self._floor:
                            self._low += 1
                            if self._low > 10:  # ~10 frames sustained
                                self.sag_tripped = True
                        else:
                            self._low = 0

    def stop(self):
        self._stop = True
        time.sleep(0.3)
        try:
            self.p.close()
        except Exception:
            pass

MSP_SET_MOTOR = 214


def set_motor(p, v):
    p.write(msp1_frame(MSP_SET_MOTOR, struct.pack("<8H", v, *([1000] * 7))))
    time.sleep(0.04)
    p.reset_input_buffer()


KISS = None  # set in main; phases read it


def kiss_cols():
    if KISS and KISS.latest:
        t, v, c = KISS.latest
        return f"  {v:5.2f}V {c:5.2f}A {t:3d}C"
    return ""


def kiss_guard():
    return KISS is not None and KISS.sag_tripped


class Guard:
    """Stall watchdog: commanded high but eRPM ~0 for too long -> abort."""

    def __init__(self, limit_s=2.0):
        self.limit = limit_s
        self.since = None
        self.tripped = False

    def feed(self, commanded, erpm):
        if commanded >= 1100 and erpm is not None and erpm < 500:
            self.since = self.since or time.time()
            if time.time() - self.since > self.limit:
                self.tripped = True
        else:
            self.since = None
        return self.tripped


def phase_ladder(p, poles, up):
    rungs = list(range(1100, 2001, 20))
    if not up:
        # start descent at 96% — back-to-back 100% dwells exceed what the
        # bench supply path holds (supply-limited, not ESC-limited)
        rungs = [r for r in rungs[::-1] if r <= 1960]
    rows = []
    guard = Guard()
    for rung in rungs:
        pct = (rung - 1000) / 10.0
        dwell = 1.2 if rung >= 1900 else 1.8  # short top-end dwell (no vbat eye)
        samples = []
        t0 = time.time()
        while time.time() - t0 < dwell:
            set_motor(p, rung)
            t = motor_telemetry(p, poles)
            if t:
                samples.append(t)
                if guard.feed(rung, t[0]):
                    return rows, f"stall at {pct:.0f}%"
            if kiss_guard():
                return rows, f"VBAT SAG guard at {pct:.0f}%"
            time.sleep(0.04)
        tail = samples[len(samples) // 2 :] or [(0, 100.0)]
        mean = sum(s[0] for s in tail) / len(tail)
        inv = max(s[1] for s in tail)
        lo = min(s[0] for s in tail)
        rows.append((pct, mean, inv, lo))
    return rows, None


def phase_fast_ramp(p, poles):
    """Continuous 10->100->10, ~24 units per MSP cycle (~4 s per leg)."""
    traj = []
    guard = Guard(limit_s=1.5)
    for leg in (range(1100, 2001, 24), range(2000, 1099, -24)):
        for rung in leg:
            set_motor(p, rung)
            t = motor_telemetry(p, poles)
            if t:
                traj.append(((rung - 1000) / 10.0, t[0], t[1]))
                if guard.feed(rung, t[0]):
                    return traj, "stall mid-ramp"
            if kiss_guard():
                return traj, "VBAT SAG guard mid-ramp"
    return traj, None


def phase_steps(p, poles, cycles=3, expect_100=None):
    events = []
    guard = Guard(limit_s=2.5)
    # establish 20% running
    t0 = time.time()
    while time.time() - t0 < 3:
        set_motor(p, 1200)
        t = motor_telemetry(p, poles)
        if t and t[0] > 4000:
            break
        time.sleep(0.04)
    for c in range(cycles):
        for target, hold in ((2000, 2.2), (1200, 2.2)):
            tgt_pct = (target - 1000) / 10.0
            samples = []
            t_step = time.time()
            reached = None
            while time.time() - t_step < hold:
                set_motor(p, target)
                t = motor_telemetry(p, poles)
                if t:
                    samples.append(t)
                    if guard.feed(target, t[0]):
                        return events, f"stall in step cycle {c + 1}"
                    if kiss_guard():
                        return events, f"VBAT SAG guard in cycle {c + 1}"
                    band = expect_100 if target == 2000 else 8900
                    if reached is None and band and t[0] > 0.8 * band:
                        reached = time.time() - t_step
                time.sleep(0.04)
            lo = min((s[0] for s in samples), default=0)
            inv = max((s[1] for s in samples), default=100.0)
            end = samples[-1][0] if samples else 0
            events.append((c + 1, tgt_pct, reached, end, lo, inv))
    return events, None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--poles", type=int, default=14)
    ap.add_argument("--kiss", default="COM41")
    ap.add_argument("--vfloor", type=float, default=9.0)
    a = ap.parse_args()
    global KISS
    try:
        KISS = Kiss(a.kiss, a.vfloor)
        KISS.start()
    except Exception as e:
        print(f"(no KISS channel: {e})")
        KISS = None
    p = serial.Serial(a.bf, 115_200, timeout=0.3)
    fail = None
    try:
        print("== phase A: slow ladder up ==")
        up, fail = phase_ladder(p, a.poles, up=True)
        for pct, mean, inv, lo in up:
            print(f"  {pct:5.0f}%  eRPM={mean:8.0f}  inv%={inv:5.2f}  min={lo:.0f}")
        print(f"  [kiss] min_v={KISS.min_v:.2f} max_a={KISS.max_a:.2f}" if KISS else "")
        if fail:
            raise KeyboardInterrupt
        expect_100 = up[-1][1]

        print("== phase A: slow ladder down ==")
        down, fail = phase_ladder(p, a.poles, up=False)
        for pct, mean, inv, lo in down:
            print(f"  {pct:5.0f}%  eRPM={mean:8.0f}  inv%={inv:5.2f}  min={lo:.0f}")
        if fail:
            raise KeyboardInterrupt

        print("== phase B: fast ramp up+down ==")
        traj, fail = phase_fast_ramp(p, a.poles)
        peak = max((e for _, e, _ in traj), default=0)
        worst_inv = max((i for _, _, i in traj), default=100)
        print(f"  {len(traj)} samples, peak eRPM={peak:.0f}, worst inv%={worst_inv:.2f}")
        if fail:
            raise KeyboardInterrupt

        print("== phase C: 20<->100 steps ==")
        steps, fail = phase_steps(p, a.poles, expect_100=expect_100)
        for c, tgt, reached, end, lo, inv in steps:
            r = f"{reached:.2f}s" if reached is not None else "never"
            print(f"  cycle {c} ->{tgt:3.0f}%: band in {r}, end={end:.0f}, "
                  f"min={lo:.0f}, inv%={inv:.2f}")
    except KeyboardInterrupt:
        pass
    finally:
        try:
            for _ in range(4):
                set_motor(p, 1000)
                time.sleep(0.1)
        except Exception:
            pass
        p.close()

    print("\n=== S50 FULL QUAL (stock AM32, BF bidir DSHOT300) ===")
    ok = fail is None
    if 'up' in dir() and up:
        mono_up = all(up[i][1] < up[i + 1][1] for i in range(len(up) - 1))
        inv_ok = all(r[2] < 1.0 for r in up)
        print(f"  ladder-up: rungs={len(up)} monotone={mono_up} inv<1%={inv_ok} "
              f"top eRPM={up[-1][1]:.0f}")
        ok = ok and mono_up and inv_ok
    if 'down' in dir() and down:
        mono_dn = all(down[i][1] > down[i + 1][1] for i in range(len(down) - 1))
        inv_ok_d = all(r[2] < 1.0 for r in down)
        print(f"  ladder-down: monotone={mono_dn} inv<1%={inv_ok_d}")
        ok = ok and mono_dn and inv_ok_d
    if 'traj' in dir() and traj:
        ramp_ok = peak > 0.9 * (up[-1][1] if up else 0) and worst_inv < 2.0
        print(f"  fast-ramp: peak within band={ramp_ok}")
        ok = ok and ramp_ok
    if 'steps' in dir() and steps:
        reach_ok = all(r is not None for _, _, r, _, _, _ in steps)
        no_zero = all(lo > 500 or tgt < 25 for _, tgt, _, _, lo, _ in steps)
        worst_reach = max((r for _, _, r, _, _, _ in steps if r is not None),
                          default=None)
        print(f"  steps: all reached band={reach_ok} no dropout={no_zero} "
              f"worst reach={worst_reach}")
        ok = ok and reach_ok and no_zero
    if fail:
        print(f"  !! {fail}")
    print(f"  verdict: {'PASS' if ok else 'CHECK'}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
