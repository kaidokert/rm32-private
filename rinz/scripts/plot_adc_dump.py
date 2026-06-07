#!/usr/bin/env python3
"""Plot an ADC capture dumped by the `d` command in examples/scope.rs.

The firmware prints, per dump:

    dump: <N> samples (8-bit, sample>>4)
    <N hex bytes, 32 per line>
    end

Each byte is the top 8 bits of a 12-bit ADC sample (raw_12bit = byte << 4), so a
value of 0xff means the raw conversion was >= 4080 (near the 4095 full-scale).

Note: the firmware ends the dump with a bare "\\r", so on a raw serial capture the
final partial line and the word "end" get merged (e.g. "end00 1e ..."). The parser
strips "end" wherever it appears, so those trailing samples are still recovered.

Usage:
    python plot_adc_dump.py [path/to/dump.log]
"""

import re
import sys
from pathlib import Path

import matplotlib.pyplot as plt

# scope.rs now captures one ADC frame per 20 kHz PWM period via TIM1_TRGO.
SAMPLE_HZ = 20_000
FRAME_HZ = 20_000
VREF = 3.3
FULL_SCALE = 255  # 8-bit after the >>4 shift
CHANNEL_PLOT_OFFSET = 4


def parse_dump(text):
    lines = text.splitlines()
    try:
        start = next(
            i
            for i, l in enumerate(lines)
            if l.lstrip().startswith("dump:") or l.lstrip().startswith("dump3:")
        )
    except StopIteration:
        raise SystemExit("no dump header found in file")

    header = lines[start].lstrip()
    is_dump3 = header.startswith("dump3:")
    m = re.search(r"dump3?:\s*(\d+)", header)
    expected = int(m.group(1)) if m else None

    body = []
    for l in lines[start + 1 :]:
        body.append(l)
        if "end" in l:
            break

    # "end" merges with the trailing samples thanks to the bare \r; drop it.
    blob = " ".join(body).replace("end", " ")
    toks = [t for t in blob.split() if re.fullmatch(r"[0-9a-fA-F]{2}", t)]
    vals = [int(t, 16) for t in toks]
    if not is_dump3:
        return [vals], expected, ["ch17"], SAMPLE_HZ

    usable = len(vals) - (len(vals) % 3)
    vals = vals[:usable]
    return [vals[i::3] for i in range(3)], expected, ["ch17", "ch5", "ch14"], FRAME_HZ


def main():
    if len(sys.argv) > 1:
        path = Path(sys.argv[1])
    else:
        path = Path(__file__).resolve().parent.parent / "logs" / "adc_dump1.log"

    channels, expected, labels, sample_hz = parse_dump(path.read_text())
    n = len(channels[0]) if channels else 0
    if not n:
        raise SystemExit("no samples parsed")
    if expected is not None and expected != n:
        print(f"warning: header said {expected} frames/samples, parsed {n}")

    flat = [v for ch in channels for v in ch]
    sat_total = sum(1 for v in flat if v == FULL_SCALE)
    print(
        f"parsed {n} frames/samples x {len(channels)} channel(s) | "
        f"min={min(flat)} max={max(flat)} mean={sum(flat) / len(flat):.1f} | "
        f"0xff (saturated)={sat_total}"
    )

    t_ms = [i / sample_hz * 1e3 for i in range(n)]

    fig, ax = plt.subplots(figsize=(14, 5))
    for ch_i, (vals, label) in enumerate(zip(channels, labels)):
        offset = ch_i * CHANNEL_PLOT_OFFSET if len(channels) > 1 else 0
        shifted = [v + offset for v in vals]
        suffix = f" +{offset}" if offset else ""
        ax.plot(t_ms[: len(vals)], shifted, lw=0.8, label=f"{label}{suffix} (8-bit)")
    for ch_i, (vals, label) in enumerate(zip(channels, labels)):
        offset = ch_i * CHANNEL_PLOT_OFFSET if len(channels) > 1 else 0
        sat = [i for i, v in enumerate(vals) if v == FULL_SCALE]
        if sat:
            ax.scatter(
                [t_ms[i] for i in sat],
                [vals[i] + offset for i in sat],
                s=16,
                zorder=3,
                label=f"{label} 0xff ({len(sat)})",
            )
    ax.set_xlabel(f"time (ms)  @ {sample_hz / 1e3:.1f} kframes/s")
    ax.set_ylabel("ADC value (top 8 of 12 bits)")
    ax.set_ylim(-5, 260 + CHANNEL_PLOT_OFFSET * max(0, len(channels) - 1))
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right")

    ax2 = ax.twinx()
    ax2.set_ylim(-5 / FULL_SCALE * VREF, 260 / FULL_SCALE * VREF)
    ax2.set_ylabel(f"~ voltage (V, Vref={VREF})")

    ax.set_title(f"{path.name}: {n} samples")
    fig.tight_layout()

    out = path.with_suffix(".png")
    fig.savefig(out, dpi=120)
    print(f"wrote {out}")
    plt.show()


if __name__ == "__main__":
    main()
