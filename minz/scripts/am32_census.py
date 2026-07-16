#!/usr/bin/env python3
"""AM32 spike census: throttle ladder + SPK-line/KISS dual logger.

Companion to the SPIKE_STATS instrumentation added to AM32 main.c
(2026-07-16): AM32 emits an ASCII "SPK n=X ms=X dep=X base=X" line
every 2 s on the telemetry UART, interleaved with the 10-byte KISS
frames BF requests. This script drives the ladder and separates the
two streams: KISS frames CRC-gate; SPK lines regex-gate.

Census definition parity with minz: events = clusters of 1 ms windows
whose 20 kHz-sampled current max exceeds baseline + 56 raw counts
(+1.5 A); dep = worst depth above baseline in raw counts (37.2/A).

Usage:
  python scripts/am32_census.py --rungs 30,40,45,50 --dwell 20
"""

import argparse
import pathlib
import re
import statistics as st
import sys
import time

import serial
import threading

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_ref import Throttle, parse_kiss  # noqa: E402


def paced_write(ser, data, gap=0.002):
    """L431 USART2 has no RX FIFO and the AM32 main-loop poll can't
    keep up with back-to-back 2 Mbaud bytes (measured: 1 of 3 dropped)
    - pace every byte."""
    for b in data:
        ser.write(bytes([b]))
        import time as _t

        _t.sleep(gap)


def bootloader_spray(ser, secs=3.0):
    """The AM32 bootloader only second-chance-jumps to the app after
    receiving GARBAGE bytes on the signal pin (BF's DSHOT provided
    them; a quiet idle-high UART line never does). Spray after any
    reset/power-up."""
    import time as _t

    end = _t.monotonic() + secs
    while _t.monotonic() < end:
        ser.write(b"U" * 16)
        _t.sleep(0.02)


class UartThrottle(threading.Thread):
    """UART_DUTY_MODE driver: ASCII percent + LF on the SAME serial
    port the telemetry arrives on (FTDI TX -> J3 S, RX -> TX1 = the
    standard minz rig). Re-sends inside the firmware's 3 s deadman."""

    def __init__(self, ser):
        super().__init__(daemon=True)
        self.ser = ser
        self.value_pct = 0
        self.run_flag = True

    def run(self):
        while self.run_flag:
            try:
                self.ser.write(f"{int(self.value_pct)}\n".encode())
            except Exception:
                pass
            import time as _t

            _t.sleep(1.0)

    def stop(self):
        self.value_pct = 0
        for _ in range(3):
            self.ser.write(b"0\ns")
            import time as _t

            _t.sleep(0.1)
        self.run_flag = False

SPK_RE = re.compile(rb"SPK n=(\d+) ms=(\d+) dep=(\d+) b=(\d+)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf-port", default="COM42")
    ap.add_argument("--tlm-port", default="COM41")
    ap.add_argument("--rungs", default="30,40,45,50")
    ap.add_argument("--dwell", type=float, default=20.0)
    ap.add_argument("--max-amps", type=float, default=5.0)
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--spray", action="store_true",
                    help="bootloader unstick spray before starting")
    ap.add_argument("--uart", action="store_true",
                    help="UART_DUTY_MODE build: throttle over the telemetry port itself")
    ap.add_argument("--tag", default="am32census")
    a = ap.parse_args()
    rungs = [int(x) for x in a.rungs.split(",")]
    capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
    stamp = time.strftime("%H%M%S")
    csv = open(capdir / f"{a.tag}_{stamp}.csv", "w")
    csv.write(
        "throttle_pct,f_e_hz,amps,volts,spk_events,spk_ms,spk_dep_raw,"
        "spk_base_raw,spk_lines,secs\n"
    )
    tlm = serial.Serial(a.tlm_port, a.baud, timeout=0.05)
    if a.spray:
        bootloader_spray(tlm)
        time.sleep(0.5)
    thr = None
    if not a.uart:
        thr = Throttle(a.bf_port)
        thr.start()
    try:
        print("idle hold 5s (clean arm)...", flush=True)
        if not a.uart:
            thr.value = 1000
        time.sleep(5.0)
        last_erpm = None
        if a.uart:
            # START VALIDATION: a failed start latches AM32's stuck-
            # rotor protection (input forced 0) ~4.5 s in, and only a
            # zero-throttle commit clears it - so a bad first rung
            # silently zeroes the whole ladder. Verify spin before
            # laddering; clear + retry on failure.
            first = rungs[0]
            for attempt in range(4):
                paced_write(tlm, f"{first}\n".encode())
                t0 = time.monotonic()
                ok = False
                while time.monotonic() - t0 < 6.0:
                    fr = [f for f in parse_kiss(bytes(tlm.read(8192)))
                          if 3.0 < f[1] < 12.0 and f[2] < 20.0]
                    if fr and fr[-1][4] > 18000:  # >300 Hz = started
                        ok = True
                        break
                if ok:
                    print(f"started at {first}% (attempt {attempt+1})", flush=True)
                    break
                print(f"start attempt {attempt+1} failed - clearing latch", flush=True)
                paced_write(tlm, b"0\n")
                time.sleep(2.0)
            else:
                raise SystemExit("motor would not start")
        for pct in rungs:
            if a.uart:
                paced_write(tlm, f"{pct}\n".encode())
            else:
                thr.value = 1000 + 10 * pct
            if a.uart:
                # keepalive through the settle too (3 s firmware deadman)
                for _ in range(2):
                    time.sleep(0.9)
                    paced_write(tlm, f"{pct}\n".encode())
            else:
                time.sleep(2.0)
            tlm.reset_input_buffer()
            buf = bytearray()
            end = time.monotonic() + a.dwell
            dead = False
            last_ka = 0.0
            while time.monotonic() < end:
                if a.uart and time.monotonic() - last_ka > 0.8:
                    paced_write(tlm, f"{pct}\n".encode())
                    last_ka = time.monotonic()
                buf += tlm.read(4096)
                r = [f for f in parse_kiss(bytes(buf[-40:]))
                     if 3.0 < f[1] < 12.0 and f[2] < 20.0 and f[0] < 100]
                if r:
                    if r[-1][2] > a.max_amps:
                        print(f"  !! {r[-1][2]:.2f} A - abort", flush=True)
                        dead = True
                        break
                    if last_erpm and last_erpm > 24000 and r[-1][4] < last_erpm * 0.5:
                        print(
                            f"  !! eRPM collapse (REBOOT) - abort", flush=True
                        )
                        dead = True
                        break
            if dead:
                if a.uart:
                    thr.value_pct = 0
                    tlm.write(b"0\ns")
                else:
                    thr.value = 1000
                break
            frames = [f for f in parse_kiss(bytes(buf))
                      if 3.0 < f[1] < 12.0 and f[2] < 20.0 and f[0] < 100]
            spk = SPK_RE.findall(bytes(buf))
            if frames:
                amps = st.median(f[2] for f in frames)
                volts = st.median(f[1] for f in frames)
                erpm = st.median(f[4] for f in frames)
                last_erpm = erpm
                fe = erpm / 60.0
            else:
                amps = volts = fe = 0.0
            ev = sum(int(m[0]) for m in spk)
            ems = sum(int(m[1]) for m in spk)
            dep = max((int(m[2]) for m in spk), default=0)
            base = st.median([int(m[3]) for m in spk]) if spk else 0
            secs = 2.0 * len(spk)
            rate = ev / secs if secs else float("nan")
            print(
                f"  thr {pct:3d}%: f_e={fe:6.0f} Hz I={amps:5.2f} A "
                f"V={volts:5.2f} | spikes={ev} ({rate:4.2f}/s) "
                f"over-ms={ems} depth={dep} raw (~{dep/37.2:.1f} A) "
                f"base={base:.0f} [{len(spk)} SPK lines]",
                flush=True,
            )
            csv.write(
                f"{pct},{fe:.0f},{amps:.2f},{volts:.2f},{ev},{ems},{dep},"
                f"{base:.0f},{len(spk)},{secs:.0f}\n"
            )
    finally:
        print("throttle down + release", flush=True)
        if a.uart:
            paced_write(tlm, b"0\ns")
        elif thr:
            thr.stop()
        csv.close()
    print(f"csv: captures/{a.tag}_{stamp}.csv")


if __name__ == "__main__":
    main()
