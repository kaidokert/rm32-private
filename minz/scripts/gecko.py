#!/usr/bin/env python3
"""GECKO-scope: capture + render the free-run OVERSAMPLED current ring.

The hybrid ADC runs the injected group (A/B/current/vbat, mid-ON, for
control) alongside a REGULAR free-run group that samples ch8 (current)
continuously — ~150 samples per 24 kHz PWM cycle — into a circular DMA
ring (adc_sync::CUR_RING, 2048 samples ≈ 0.55 ms). This is the
intra-cycle current microscope: it resolves the shape of a
current-spike event at ~0.27 µs, distinguishing a smooth
winding-limited BEMF-aided ramp (4-5 A over hundreds of µs) from a
sub-µs shoot-through transient (ADC-railed) at a switching edge.

Two ways to get a dump:

  * `--force` / key `G`: on-demand snapshot of the ring right now.
  * armed auto-trigger (firmware `J` key toggles WAX_TRIG_ARMED): the
    ISR freezes the ring the instant a >4 A cycle sample is seen, so
    the ring holds the ~0.55 ms LEADING UP TO the spike — the ONSET.
    Run this with `--wait` after arming the board (`J`).

Wire format (same a85 cdump framing as WAXWING):
    gecko: 2048 samples b85 isns oversampled 12-bit, adc_hz~3700000
    <ascii85 body: 512 frames x 4 u16 = 2048 samples, oldest-first>
    end
The auto-trigger path is followed by a normal `j` WAXWING dump (the
per-cycle A/B/comp/sector context); this tool ignores it.

Usage:
    python scripts/gecko.py --force --tag baseline
    python scripts/gecko.py --wait 20 --tag spike      # after board `J`
    python scripts/gecko.py --infile captures/gecko_x.txt   # re-render
"""

import argparse
import base64
import pathlib
import struct
import time

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

# Sense calibration (motor_tester2 / CLAUDE.md): INA180x1 (20 V/V) x
# 1.5 mOhm shunt -> 30 mA per mV, 12-bit over 3.3 V.
ISNS_MV_PER_AMP = 30.0
ADC_MV_PER_COUNT = 3300.0 / 4096.0
ADC_HZ = 3_700_000.0  # ~ch8 12.5-cycle free-run rate; refined from header


def counts_to_amps(counts):
    mv = counts * ADC_MV_PER_COUNT
    return mv / ISNS_MV_PER_AMP


def parse_gecko(text):
    """Return (samples_u16, adc_hz) from a gecko dump text blob."""
    lines = text.splitlines()
    hdr_i = next(i for i, l in enumerate(lines) if l.lstrip().startswith("gecko:"))
    header = lines[hdr_i].strip()
    n_samples = int(header.split()[1])
    adc_hz = ADC_HZ
    for tok in header.split():
        if tok.startswith("adc_hz"):
            # "adc_hz~3700000"
            try:
                adc_hz = float(tok.split("~")[1].rstrip(","))
            except (IndexError, ValueError):
                pass
    body = []
    for l in lines[hdr_i + 1 :]:
        if l.strip() == "end":
            break
        s = l.strip()
        if s:
            body.append(s)
    raw = base64.a85decode("".join(body).encode("ascii"))
    vals = struct.unpack(f"<{len(raw) // 2}H", raw[: len(raw) // 2 * 2])
    vals = np.array(vals[:n_samples], dtype=np.uint16)
    return vals, adc_hz


def capture(port, baud, force, wait_s):
    import serial

    with serial.Serial(port, baud, timeout=0.05) as p:
        p.reset_input_buffer()
        if force:
            p.write(b"G")
        deadline = time.time() + (wait_s if wait_s else 8.0)
        buf = b""
        while time.time() < deadline:
            chunk = p.read(65536)
            if chunk:
                buf += chunk
                if b"gecko:" in buf and (b"\nend" in buf or b"\rend" in buf):
                    # got at least one full gecko dump
                    if buf.count(b"end") >= 1:
                        break
        return buf.decode("ascii", errors="replace")


def render(vals, adc_hz, stem, out_png):
    amps = counts_to_amps(vals.astype(np.float64))
    dt_us = 1e6 / adc_hz
    t_us = np.arange(len(vals)) * dt_us
    # PWM cycle length for gridlines: 24 kHz -> 41.67 µs.
    pwm_us = 1e6 / 24000.0

    fig, (ax, axz) = plt.subplots(2, 1, figsize=(15, 8), sharex=False)
    railed = int((vals >= 4090).sum())
    peak = amps.max()
    ax.plot(t_us, amps, lw=0.6, color="tab:red")
    ax.axhline(4.0, color="k", ls="--", lw=0.8, label="4 A trigger")
    for x in np.arange(0, t_us[-1], pwm_us):
        ax.axvline(x, color="0.85", lw=0.4, zorder=0)
    ax.set_ylabel("current (A)")
    ax.set_title(
        f"{stem}  |  {len(vals)} samples @ {adc_hz/1e6:.2f} MHz "
        f"({dt_us:.2f} µs/samp, {len(vals)*dt_us:.0f} µs span)  |  "
        f"peak {peak:.1f} A  railed(>=4090) {railed}  |  grey = 24 kHz PWM wraps"
    )
    ax.legend(fontsize=8, loc="upper right")
    ax.grid(alpha=0.25)

    # Zoom: the last ~4 PWM cycles (where an auto-trigger fires).
    zwin = min(len(vals), int(4 * pwm_us / dt_us))
    axz.plot(t_us[-zwin:], amps[-zwin:], lw=0.9, color="tab:red", marker=".", ms=2)
    axz.axhline(4.0, color="k", ls="--", lw=0.8)
    for x in np.arange(0, t_us[-1], pwm_us):
        if x >= t_us[-zwin]:
            axz.axvline(x, color="0.8", lw=0.5, zorder=0)
    axz.set_ylabel("current (A)")
    axz.set_xlabel("time (µs)")
    axz.set_title(f"zoom: last {zwin} samples (~4 PWM cycles) — intra-cycle shape")
    axz.grid(alpha=0.25)

    fig.tight_layout()
    fig.savefig(out_png, dpi=110)
    print(f"peak {peak:.2f} A, mean {amps.mean():.2f} A, railed {railed}/{len(vals)}")
    print(f"wrote {out_png}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--force", action="store_true", help="send G for an on-demand dump")
    ap.add_argument("--wait", type=float, default=0.0, help="seconds to wait for an armed trigger dump")
    ap.add_argument("--tag", default="gecko")
    ap.add_argument("--infile", default=None, help="re-render an existing capture")
    args = ap.parse_args()

    capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
    capdir.mkdir(exist_ok=True)

    if args.infile:
        text = pathlib.Path(args.infile).read_text(errors="replace")
        stem = pathlib.Path(args.infile).stem
    else:
        text = capture(args.port, args.baud, args.force, args.wait)
        stamp = time.strftime("%H%M%S")
        stem = f"{args.tag}_{stamp}"
        outp = capdir / f"{stem}.txt"
        outp.write_text(text)
        print(f"saved {len(text)} chars to {outp}")

    vals, adc_hz = parse_gecko(text)
    print(f"parsed {len(vals)} current samples @ {adc_hz:.0f} Hz")
    render(vals, adc_hz, stem, str(capdir / f"{stem}.png"))


if __name__ == "__main__":
    main()
