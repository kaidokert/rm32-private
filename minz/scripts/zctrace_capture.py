"""AM32 ZC-trace capture + decode (differential climb trace, AM32 side).

Drives the UART_DUTY_MODE throttle with keepalives, captures the raw
wire (trace records + KISS + SPK interleaved), decodes the 15-byte
5B A9 trace records, and writes a CSV.

Record (LE): 5B A9 | flags_step | thiszctime u16 (0.5us ticks) |
commutation_interval u16 | waitTime u16 | duty u16 | tenkhz u16 |
average_interval u16.  flags_step: bits0-2 step, bit7 old_routine.

Trace regime: <=80% throttle (higher saturates the 2M wire - by
design; the differential experiment targets the 60->80 climb).

Usage:
  python scripts/zctrace_capture.py --profile 60:3,62:0.7,...,80:8 \
      --tag zct1
  python scripts/zctrace_capture.py --climb 60 80 --dwell 3 --tag zct1
"""

import argparse
import pathlib
import struct
import sys
import time

import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write, bootloader_spray  # noqa: E402
from am32_ref import parse_kiss  # noqa: E402

PORT = "COM41"
BAUD = 2_000_000
REC = 15


def decode(buf):
    recs = []
    i = 0
    while i + REC <= len(buf):
        if buf[i] == 0x5B and buf[i + 1] == 0xA9:
            fs = buf[i + 2]
            (zt, ci, wt, duty, tk, avg) = struct.unpack_from(
                "<HHHHHH", buf, i + 3)
            # sanity: raw period 40..30000 ticks (20us..15ms)
            if 40 <= zt <= 30000 and ci <= 30000:
                recs.append(dict(step=fs & 7, old=bool(fs & 0x80),
                                 zt_ticks=zt, ci_ticks=ci, wait_ticks=wt,
                                 duty=duty, tenkhz=tk, avg_ticks=avg))
                i += REC
                continue
        i += 1
    return recs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--climb", nargs=2, type=int, default=[60, 80])
    ap.add_argument("--dwell", type=float, default=3.0)
    ap.add_argument("--step-wait", type=float, default=0.7)
    ap.add_argument("--tag", default="zct")
    ap.add_argument("--spray", action="store_true")
    a = ap.parse_args()
    lo, hi = a.climb
    assert hi <= 80, "trace regime is <=80% (wire saturates above)"

    ser = serial.Serial(PORT, BAUD, timeout=0.05)
    cap = bytearray()

    def hold(pct, secs):
        t0 = time.monotonic()
        last_ka = 0.0
        while time.monotonic() - t0 < secs:
            if time.monotonic() - last_ka > 0.8:
                paced_write(ser, f"{pct}\n".encode())
                last_ka = time.monotonic()
            cap.extend(ser.read(8192))

    try:
        if a.spray:
            bootloader_spray(ser)
            time.sleep(0.5)
        # clean arm: AM32 arms on ~1s of zero throttle
        print("idle hold 5s (clean arm)...")
        hold(0, 5.0)
        # start + settle at lo
        for att in range(4):
            hold(lo, 4.0)
            fr = [f for f in parse_kiss(bytes(cap[-20000:]))
                  if 3.0 < f[1] < 12.0]
            if fr and fr[-1][4] > 18000:
                break
            paced_write(ser, b"0\n")
            time.sleep(2.0)
        else:
            raise SystemExit("no start")
        print(f"started; climbing {lo}->{hi}")
        hold(lo, a.dwell)
        for pct in range(lo + 1, hi + 1):
            hold(pct, a.step_wait)
        hold(hi, a.dwell)
    finally:
        paced_write(ser, b"0\n")
        time.sleep(1.0)
        cap.extend(ser.read(8192))
        paced_write(ser, b"0\n")
        ser.close()

    raw_path = pathlib.Path("captures") / f"{a.tag}_raw.bin"
    raw_path.write_bytes(cap)
    recs = decode(bytes(cap))
    csv_path = pathlib.Path("captures") / f"{a.tag}_trace.csv"
    with open(csv_path, "w") as f:
        f.write("step,old,zt_us,ci_us,wait_us,duty,tenkhz,avg_us\n")
        for r in recs:
            f.write(f"{r['step']},{int(r['old'])},{r['zt_ticks']/2:.1f},"
                    f"{r['ci_ticks']/2:.1f},{r['wait_ticks']/2:.1f},"
                    f"{r['duty']},{r['tenkhz']},{r['avg_ticks']/2:.1f}\n")
    print(f"captured {len(cap)} B -> {len(recs)} trace records")
    print(f"raw: {raw_path}  csv: {csv_path}")
    if recs:
        mid = recs[len(recs) // 2]
        print(f"sample mid-record: {mid}")


if __name__ == "__main__":
    main()
