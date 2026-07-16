#!/usr/bin/env python3
"""AM32 reference curve: BF CLI throttle ladder + KISS telemetry log.

The reference experiment (2026-07-15): AM32 runs this exact board to
100 % throttle; we log ITS amps-per-electrical-Hz curve to price the
minz loop's timing waste in amps.

Rig:
  - Betaflight FC USB -> --bf-port (COM42; CLI `motor` drive, reusing
    the May-era bf_link machinery from E:/m/robot/esc/scripts)
  - BF motor-1 pad -> J3 "S" (PA2)   [FTDI TX must be REMOVED from S]
  - FTDI RX stays on J3 "TX1" -> --tlm-port (AM32 KISS telemetry,
    115200 8N1; BF needs DSHOT300 + feature ESC_SENSOR so the
    telemetry-request bit is set and AM32 answers on PB6)
  - Common GND everywhere. Bench PSU current limit = the hard guard.

Usage:
  python scripts/am32_ref.py --rungs 20,30,40,50,60,70,80,90,100 --dwell 6
"""

import argparse
import pathlib
import statistics as st
import sys
import threading
import time

import serial

sys.path.insert(0, r"E:\m\robot\esc\scripts")
from bf_link import BFLink  # noqa: E402


class Throttle(threading.Thread):
    """Hold the BF CLI open and re-assert the motor value at ~5 Hz
    (bf_drive.py lesson: BF drops to disarmed the moment the CDC port
    closes, and idle watchdogs can zero a stale value)."""

    def __init__(self, port):
        super().__init__(daemon=True)
        self.link = BFLink(port)
        self.link.s.write(b"#\r\n")
        time.sleep(0.4)
        self.link.s.reset_input_buffer()
        self.value = 1000
        self.run_flag = True

    def run(self):
        while self.run_flag:
            cmd = f"motor 0 {int(self.value)}\r\n".encode()
            try:
                self.link.s.write(cmd)
                time.sleep(0.2)
                self.link.s.reset_input_buffer()
            except Exception:
                pass

    def stop(self):
        self.value = 1000
        for _ in range(5):
            self.link.s.write(b"motor 0 1000\r\n")
            time.sleep(0.1)
        self.run_flag = False
        time.sleep(0.3)
        self.link.close()  # NOT 'exit' (that reboots BF)


def kiss_crc8(buf):
    crc = 0
    for b in buf:
        crc ^= b
        for _ in range(8):
            crc = ((crc << 1) ^ 0x07) & 0xFF if crc & 0x80 else (crc << 1) & 0xFF
    return crc


def parse_kiss(buf):
    """Yield (temp_C, volts, amps, mah, erpm) for every CRC-valid
    10-byte KISS frame in buf."""
    out = []
    i = 0
    while i + 10 <= len(buf):
        f = buf[i : i + 10]
        if kiss_crc8(f[:9]) == f[9]:
            out.append(
                (
                    f[0],
                    ((f[1] << 8) | f[2]) / 100.0,
                    ((f[3] << 8) | f[4]) / 100.0,
                    (f[5] << 8) | f[6],
                    ((f[7] << 8) | f[8]) * 100,
                )
            )
            i += 10
        else:
            i += 1
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf-port", default="COM42")
    ap.add_argument("--tlm-port", default="COM41")
    ap.add_argument("--rungs", default="20,30,40,50,60,70,80,90,100")
    ap.add_argument("--dwell", type=float, default=6.0)
    ap.add_argument(
        "--max-amps",
        type=float,
        default=4.0,
        help="back off the ladder if AM32-reported current exceeds this",
    )
    ap.add_argument("--tag", default="am32ref")
    a = ap.parse_args()
    rungs = [int(x) for x in a.rungs.split(",")]
    capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
    stamp = time.strftime("%H%M%S")
    csv = open(capdir / f"{a.tag}_{stamp}.csv", "w")
    csv.write("throttle_pct,f_e_hz,erpm,amps,volts,temp_c,frames\n")

    thr = Throttle(a.bf_port)
    thr.start()
    tlm = serial.Serial(a.tlm_port, 115200, timeout=0.05)
    try:
        print("motor value 1000 hold (AM32 arm), 3 s...", flush=True)
        time.sleep(3.0)
        for pct in rungs:
            thr.value = 1000 + 10 * pct
            time.sleep(1.5)  # spin-up / settle
            tlm.reset_input_buffer()
            buf = bytearray()
            end = time.monotonic() + a.dwell
            abort = False
            while time.monotonic() < end:
                buf += tlm.read(4096)
                recent = parse_kiss(bytes(buf[-40:]))
                if recent and recent[-1][2] > a.max_amps:
                    print(
                        f"  !! {recent[-1][2]:.2f} A > --max-amps at "
                        f"{pct}% - backing off",
                        flush=True,
                    )
                    abort = True
                    break
            frames = parse_kiss(bytes(buf))
            if frames:
                amps = st.median(f[2] for f in frames)
                volts = st.median(f[1] for f in frames)
                erpm = st.median(f[4] for f in frames)
                fe = erpm / 60.0
                print(
                    f"  thr {pct:3d}%: f_e={fe:6.0f} Hz erpm={erpm:8.0f} "
                    f"I={amps:5.2f} A V={volts:5.2f} T={frames[-1][0]} C "
                    f"({len(frames)} frames)",
                    flush=True,
                )
                csv.write(
                    f"{pct},{fe:.0f},{erpm:.0f},{amps:.2f},"
                    f"{volts:.2f},{frames[-1][0]},{len(frames)}\n"
                )
            else:
                print(f"  thr {pct:3d}%: NO TELEMETRY FRAMES", flush=True)
                csv.write(f"{pct},,,,,,0\n")
            if abort:
                break
    finally:
        print("throttle down + release", flush=True)
        thr.stop()
        csv.close()
    print(f"csv: captures/{a.tag}_{stamp}.csv")


if __name__ == "__main__":
    main()
