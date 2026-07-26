#!/usr/bin/env python3
"""WAXWING-lite phase-voltage capture for am32_clone ('X' key, 2026-07-26).

Sends 'X' on the UART_DUTY_MODE link; the clone dumps its always-on
per-TIM6-tick rings (phase A/B injected mid-ON samples + position
metadata) as ASCII hex:

    WX n=1024 head=<next-write-slot> ci=<commutation_interval> arr=<tim1_arr>
    <256 lines of 4 records; each record = 4 space-separated
     4-hex-char u16s: A B POS T1S; raw ring order>
    WX END

Record fields:
    A / B  — raw 12-bit injected phase samples (ch9/PA4, ch10/PA5)
    POS    — INTERVAL_TIMER (TIM2) CNT at ring-write time, 0.5 us ticks
    T1S    — (step << 12) | (TIM1.CNT & 0x0FFF); step is AM32 1..6

The rings keep writing during the dump (TIM6 preempts main), so this
tool discards +/-8 records around the snapshot head as possibly torn.

Harvest-skew correction: the injected burst fired at carrier
CNT==SAMPLE_TICKS (100); the ring write happens up to a carrier
period later. elapsed80 = t1cnt-100 (wrapped over ARR+1), and
pos_corrected_halfus = pos - elapsed80/40 (80 MHz -> 0.5 us ticks).
Negative corrected positions mean the commutation window rolled
between sample and write — those records are flagged and excluded
from the scatter.

Outputs captures/wax_<tag>.csv and captures/wax_<tag>.png:
  panel 1 — equivalent-time scatter (x = corrected window position in
            us, y = raw phase sample; float-sector records highlighted;
            the demag clamp shows as a band pinned near 0/rail at low
            x, releasing into the BEMF arc);
  panel 2 — A and B vs record index (time series, for orientation).

NOTE: the clone's UartDuty parser takes RAW single characters — send
'X' once (the DOUBLED-key convention was motor_tester2's, not this).

Usage:
    python scripts/waxwing_grab.py --tag idle
    python scripts/waxwing_grab.py --pct 90 --settle 4 --kill --tag t90
"""

import argparse
import pathlib
import re
import sys
import time

import serial

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write  # noqa: E402

SAMPLE_TICKS = 100  # adc_sync::SAMPLE_TICKS — injected trigger point
TICKS80_PER_HALFUS = 40  # 80 MHz ticks per 0.5 us
TORN_GUARD = 8  # records discarded on each side of the head
N_EXPECTED = 1024

HDR_RE = re.compile(rb"WX n=(\d+) head=(\d+) ci=(\d+) arr=(\d+)")


def capture(ser, timeout=5.0):
    """Send 'X', read to 'WX END'. Returns (records, head, ci, arr)
    with records in raw ring order as (a, b, pos, t1s) tuples."""
    ser.reset_input_buffer()
    paced_write(ser, b"X")
    buf = b""
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        buf += ser.read(65536)
        if b"WX END" in buf:
            break
    m = HDR_RE.search(buf)
    if not m:
        raise RuntimeError("no WX header seen (wire tail: %r)" % buf[-200:])
    n, head, ci, arr = (int(m.group(i)) for i in range(1, 5))
    body = buf[m.end():buf.index(b"WX END")]
    words = [int(t, 16) for t in re.findall(rb"[0-9a-fA-F]{4}", body)]
    if len(words) < 4 * n:
        raise RuntimeError(f"short dump: {len(words)}/{4 * n} words")
    words = words[: 4 * n]
    recs = [tuple(words[4 * i: 4 * i + 4]) for i in range(n)]
    return recs, head, ci, arr


def process(recs, head, arr):
    """Reorder oldest-first from head, drop the torn guard band, unpack
    step/t1cnt, apply the harvest-skew correction. Returns a dict of
    np arrays (rolled-window records flagged, not removed)."""
    ordered = recs[head:] + recs[:head]
    ordered = ordered[TORN_GUARD:-TORN_GUARD]
    a = np.array([r[0] for r in ordered], dtype=float)
    b = np.array([r[1] for r in ordered], dtype=float)
    pos = np.array([r[2] for r in ordered], dtype=float)  # 0.5 us ticks
    t1s = np.array([r[3] for r in ordered], dtype=int)
    step = t1s >> 12  # AM32 1..6
    t1cnt = t1s & 0x0FFF
    # Skew: elapsed 80 MHz ticks since the injected trigger (carrier is
    # quasi-static at a held throttle, so the header ARR is good enough).
    elapsed80 = np.where(
        t1cnt >= SAMPLE_TICKS,
        t1cnt - SAMPLE_TICKS,
        t1cnt + arr + 1 - SAMPLE_TICKS,
    )
    pos_corr = pos - elapsed80 / TICKS80_PER_HALFUS
    rolled = pos_corr < 0  # window rolled between sample and write
    return dict(a=a, b=b, pos=pos, pos_corr=pos_corr, step=step, rolled=rolled)


def render(d, tag, ci, arr):
    cap = pathlib.Path(__file__).resolve().parent.parent / "captures"
    cap.mkdir(exist_ok=True)
    csv = cap / f"wax_{tag}.csv"
    with open(csv, "w") as f:
        f.write("idx,a,b,pos_halfus,pos_corrected,step\n")
        for i in range(len(d["a"])):
            f.write(
                f"{i},{d['a'][i]:.0f},{d['b'][i]:.0f},{d['pos'][i]:.0f},"
                f"{d['pos_corr'][i]:.1f},{d['step'][i]}\n"
            )

    ok = ~d["rolled"]
    x_us = d["pos_corr"] / 2.0  # 0.5 us ticks -> us
    sector = d["step"] - 1  # minz sector frame 0..5
    a_float = np.isin(sector, (2, 5))  # phase A floats sectors 2/5
    b_float = np.isin(sector, (1, 4))  # phase B floats sectors 1/4

    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(14, 9))
    for phase, y, is_float, marker in (
        ("A", d["a"], a_float, "o"),
        ("B", d["b"], b_float, "^"),
    ):
        for sel, color, lab in (
            (ok & is_float, "#cc3333", f"{phase} (float sector)"),
            (ok & ~is_float, "#88aadd", f"{phase} (driven)"),
        ):
            ax1.scatter(x_us[sel], y[sel], s=6, marker=marker,
                        color=color, alpha=0.6, label=lab)
    ax1.set_xlabel("corrected window position (us since ZC ref)")
    ax1.set_ylabel("raw 12-bit phase sample (mid-ON)")
    ax1.set_title(
        f"WAXWING {tag}: equivalent-time demag scatter, ci={ci} (0.5us), "
        f"arr={arr}, {ok.sum()}/{len(ok)} records ({d['rolled'].sum()} rolled)"
    )
    ax1.legend(loc="upper right", fontsize=8)
    ax1.grid(alpha=0.3)

    idx = np.arange(len(d["a"]))
    ax2.plot(idx, d["a"], lw=0.5, color="#cc3333", label="A")
    ax2.plot(idx, d["b"], lw=0.5, color="#88aadd", label="B")
    ax2.set_xlabel("record index (oldest first, 50 us/record)")
    ax2.set_ylabel("raw 12-bit")
    ax2.legend(loc="upper right")
    ax2.grid(alpha=0.3)

    png = cap / f"wax_{tag}.png"
    fig.tight_layout()
    fig.savefig(png, dpi=120)
    print(f"saved {csv}")
    print(f"saved {png}")

    # Per-step-parity counts + covered x-range.
    even = np.isin(sector % 2, (0,))
    print(f"records: {len(idx)} total, {d['rolled'].sum()} rolled (excluded)")
    print(f"sector parity: even={int(even.sum())} odd={int((~even).sum())}")
    for s in range(1, 7):
        n = int((d["step"] == s).sum())
        print(f"  step {s} (sector {s - 1}): {n}")
    if ok.any():
        print(f"x-range covered: {x_us[ok].min():.1f}..{x_us[ok].max():.1f} us")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--tag", default="grab")
    ap.add_argument("--pct", type=int, default=None,
                    help="set throttle percent before capturing")
    ap.add_argument("--settle", type=float, default=3.0,
                    help="seconds to hold --pct before 'X'")
    ap.add_argument("--kill", action="store_true",
                    help="send '0\\n' + 'w' on every exit path")
    a = ap.parse_args()

    ser = serial.Serial(a.port, a.baud, timeout=0.05)
    try:
        if a.pct is not None:
            paced_write(ser, f"{a.pct}\n".encode())
            # re-send halfway through settle (deadman zeroes throttle
            # after 3 s of silence — gecko_grab convention).
            time.sleep(a.settle / 2)
            paced_write(ser, f"{a.pct}\n".encode())
            time.sleep(a.settle / 2)
        recs, head, ci, arr = capture(ser)
        print(f"captured {len(recs)} records, head={head}, ci={ci}, arr={arr}")
        if ci > 2000:
            print("WARNING: ci > 2000 (idle?) — motor not running, "
                  "the equivalent-time scatter is meaningless")
        d = process(recs, head, arr)
        render(d, a.tag, ci, arr)
    finally:
        # Motor-script kill-guard (house rule): kill on every exit path
        # whenever the motor was (or may have been) commanded.
        if a.kill or a.pct is not None:
            try:
                paced_write(ser, b"0\n")
                time.sleep(0.2)
                paced_write(ser, b"w")
            except Exception:
                pass
        ser.close()


if __name__ == "__main__":
    main()
