#!/usr/bin/env python3
"""BEMF-RING host capture + plot (port of minz's waxwing_grab.py).

Captures the always-on floating-phase ring the `bemf-ring` firmware dumps
over VCOM after each speed rung, and renders the equivalent-time scatter that
shows the TRUE BEMF: floating-phase voltage vs position-in-commutation-window,
folded across all windows, colored by step, with the neutral and the
comparator-level markers.

Firmware dump per rung:
    RG n=1024 head=<slot> period_us=<hold>
    <1024 lines: "VF NEU POS T1S" as 4-hex u16s (raw ring order)>
    RG END
  ... (3 rungs) ...
    RG ALLDONE

Record fields:
    VF   raw 12-bit floating-phase ADC (mid-ON, this step's float phase)
    NEU  raw 12-bit 3-phase-mean virtual neutral (same DMA scan)
    POS  TIM2 CNT at ring-write = 0.5 us since the last commutation
    T1S  (step<<12) | (TIM1.CNT & 0x0FFF) | (comp_level<<15)

Read from the plot:
  - the demag spike right after commutation (low POS)  -> sets BLANK
  - the noise band where VF crosses NEU                 -> sets HYST
  - the POS where VF crosses NEU (the true ZC angle)
  - VF swing growing with speed across rungs            -> proves real BEMF

Usage:
    python scripts/bemf_ring.py            # capture live + plot
    python scripts/bemf_ring.py --infile captures/bemf_ring.txt
"""
import argparse
import pathlib
import time

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

PORT = "COM7"
BAUD = 2_000_000
ADC_TO_MV = 3300.0 / 4096.0

ap = argparse.ArgumentParser()
ap.add_argument("--infile", default=None)
ap.add_argument("--secs", type=float, default=25.0)
ap.add_argument("--tag", default="bemf_ring")
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

if args.infile:
    text = pathlib.Path(args.infile).read_text(encoding="latin-1")
else:
    import serial
    with serial.Serial(PORT, BAUD, timeout=0.2) as p:
        buf = bytearray()
        t0 = time.time()
        while time.time() - t0 < args.secs:
            buf += p.read(65536)
            if b"RG ALLDONE" in buf:
                break
    text = buf.decode("latin-1", "replace")
    (capdir / f"{args.tag}.txt").write_text(text)
    print(f"captured {len(text)} chars -> captures/{args.tag}.txt")

# --- parse rungs ---
lines = text.splitlines()
rungs = []
i = 0
while i < len(lines):
    ln = lines[i]
    if ln.startswith("RG n="):
        hdr = dict(kv.split("=") for kv in ln[3:].split() if "=" in kv)
        n = int(hdr.get("n", 1024))
        head = int(hdr.get("head", 0))
        period = int(hdr.get("period_us", 0))
        recs = []
        i += 1
        while i < len(lines) and not lines[i].startswith("RG END"):
            p = lines[i].split()
            if len(p) == 4:
                try:
                    recs.append([int(x, 16) for x in p])
                except ValueError:
                    pass
            i += 1
        if recs:
            arr = np.array(recs, dtype=np.int32)
            # reorder from head so index 0 = oldest (not required for scatter)
            rungs.append((period, head, arr))
    i += 1

if not rungs:
    print("NO RUNGS PARSED — check COM7 / firmware")
    raise SystemExit(1)

print(f"parsed {len(rungs)} rungs: periods={[r[0] for r in rungs]} us")

# --- plot ---
fig, axes = plt.subplots(len(rungs), 1, figsize=(11, 3.2 * len(rungs)), squeeze=False)
step_colors = plt.cm.tab10(np.arange(6))

for ri, (period, head, arr) in enumerate(rungs):
    ax = axes[ri][0]
    vf = arr[:, 0] * ADC_TO_MV
    neu = arr[:, 1] * ADC_TO_MV
    pos_us = arr[:, 2] * 0.5  # 0.5 us ticks
    t1s = arr[:, 3]
    step = (t1s >> 12) & 0x7
    comp = (t1s >> 15) & 0x1

    # only rows with a valid step (1..6) and plausible pos
    m = (step >= 1) & (step <= 6) & (pos_us < 30000)
    for s in range(1, 7):
        sm = m & (step == s)
        if sm.any():
            ax.scatter(pos_us[sm], vf[sm], s=6, color=step_colors[s - 1],
                       label=f"step{s}", alpha=0.6)
    # neutral (mean of measured neutral)
    ax.axhline(np.median(neu[m]), color="k", ls="--", lw=1,
               label=f"neutral~{np.median(neu[m]):.0f}mV")
    # comp==1 markers along the bottom to show where the detector fired high
    cm = m & (comp == 1)
    ax.scatter(pos_us[cm], np.full(cm.sum(), vf[m].min() if m.any() else 0),
               s=3, color="red", marker="|", alpha=0.3, label="comp=hi")
    ax.set_title(f"rung {ri}: forced {period} us/step  (VF float-phase vs position-in-window)")
    ax.set_xlabel("position since commutation (us)")
    ax.set_ylabel("VF / neutral (mV)")
    ax.grid(alpha=0.3)
    ax.legend(fontsize=7, ncol=4, loc="upper right")

fig.tight_layout()
out = capdir / f"{args.tag}.png"
fig.savefig(out, dpi=110)
print(f"saved plot -> {out}")

# --- text summary (so it's useful even without opening the PNG) ---
print("\n=== per-trigger VF swing (BEMF is where swing is LARGE + step-separated) ===")
print("  trig(cnt)  VF_min..max(mV)  swing  neutral  %below  (step spread)")
for ri, (trig, head, arr) in enumerate(rungs):
    vf = arr[:, 0] * ADC_TO_MV
    neu = arr[:, 1] * ADC_TO_MV
    t1s = arr[:, 3]
    step = (t1s >> 12) & 0x7
    m = (step >= 1) & (step <= 6)
    if not m.any():
        continue
    swing = vf[m].max() - vf[m].min()
    nmed = np.median(neu[m])
    frac_below = (vf[m] < nmed).mean()
    # per-step medians: if BEMF is real, the floating phase differs step-to-step
    smeds = [np.median(vf[m & (step == s)]) if (m & (step == s)).any() else 0 for s in range(1, 7)]
    spread = max(smeds) - min(smeds)
    print(f"  {trig:5d}    {vf[m].min():5.0f}..{vf[m].max():5.0f}    {swing:5.0f}   "
          f"{nmed:5.0f}   {100*frac_below:3.0f}%   step-spread={spread:.0f}mV")
