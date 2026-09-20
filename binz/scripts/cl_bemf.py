#!/usr/bin/env python3
"""Closed-loop death-capture plotter — reads WHY the BEMF lock slips.

The `closed-loop` firmware keeps a 1024-deep ring of what the software
comparator saw + decided every 20 kHz tick, and dumps it over VCOM as ASCII
hex at three moments:
    tag=a11e  synced-reference, locked at 20% BEFORE the throttle walk
    tag=600d  success — reached and held 30%
    tag=dead  the kill — ring FROZEN on the guard trip, so its tail is the slip

Each block:
    CLBEMF n=1024 head=<slot> tag=<hex> ci=<> dc=<> vm=<> kr=<killreason>
    <1024 lines: "VF NEU POS FLG" as 4-hex u16s (raw ring order)>
    CLBEMF END

Fields:
    VF   floating-phase mV the detector compared     (VF_DIAG)
    NEU  per-sector self-cal neutral mV it compared  (NEUTRAL_DIAG)
    POS  TIM2 CNT = 0.5 us since the last commutation
    FLG  step(b0..2) | level(b3) | edge_latched(b4) | zc_accepted(b5)
         | engaged(b6) | (vm_pin_mV>>2)<<7   [coarse bus]

The primary read is the TIME view (top of each block): VF and NEU vs tick
across the capture, with green ticks where a ZC was ACCEPTED and the coarse
bus underneath. A healthy lock = VF swinging cleanly through NEU, a steady
accept cadence, flat bus. The slip = accepts thin out / stop, VF stops
crossing NEU (or NEU runs away from VF), and the bus collapses — and you SEE
which happens first, which is the mechanism.

Usage:
    python scripts/cl_bemf.py                 # capture live COM7 + plot
    python scripts/cl_bemf.py --infile captures/cl_bemf.txt
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
DT_US = 50.0  # 20 kHz tick

ap = argparse.ArgumentParser()
ap.add_argument("--infile", default=None)
ap.add_argument("--secs", type=float, default=45.0)
ap.add_argument("--tag", default="cl_bemf")
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
        seen_terminal = False
        while time.time() - t0 < args.secs:
            buf += p.read(65536)
            # stop once a terminal (kill/success) block has fully arrived
            if (b"tag=dead" in buf or b"tag=600d" in buf) and buf.count(b"CLBEMF END") >= 2:
                seen_terminal = True
                break
        _ = seen_terminal
    text = buf.decode("latin-1", "replace")
    (capdir / f"{args.tag}.txt").write_text(text)
    print(f"captured {len(text)} chars -> captures/{args.tag}.txt")

# --- parse blocks ---
lines = text.splitlines()
blocks = []
i = 0
while i < len(lines):
    ln = lines[i]
    if ln.startswith("CLBEMF n="):
        hdr = dict(kv.split("=") for kv in ln[6:].split() if "=" in kv)
        head = int(hdr.get("head", 0))
        tag = hdr.get("tag", "????")
        meta = {k: hdr.get(k) for k in ("ci", "dc", "vm", "kr")}
        recs = []
        i += 1
        while i < len(lines) and not lines[i].startswith("CLBEMF END"):
            parts = lines[i].split()
            if len(parts) == 4:
                try:
                    recs.append([int(x, 16) for x in parts])
                except ValueError:
                    pass
            i += 1
        if len(recs) > 16:
            arr = np.array(recs, dtype=np.int32)
            # reorder so 0 = oldest (ring order: head points at next write = oldest)
            if len(arr) >= head:
                arr = np.concatenate([arr[head:], arr[:head]])
            blocks.append((tag, meta, arr))
    i += 1

if not blocks:
    print("NO CLBEMF BLOCKS PARSED — check COM7 / firmware")
    raise SystemExit(1)

TAGNAME = {"a11e": "SYNCED REF @20%", "600d": "SUCCESS @30%", "dead": "THE SLIP (kill)"}
print(f"parsed {len(blocks)} blocks: {[b[0] for b in blocks]}")

step_colors = plt.cm.tab10(np.arange(6))
fig, axes = plt.subplots(len(blocks), 3, figsize=(17, 3.6 * len(blocks)), squeeze=False)

for bi, (tag, meta, arr) in enumerate(blocks):
    vf = arr[:, 0].astype(float)
    neu = arr[:, 1].astype(float)
    pos_us = arr[:, 2] * 0.5
    flg = arr[:, 3]
    step = flg & 0x7
    level = (flg >> 3) & 1
    edge = (flg >> 4) & 1
    acc = (flg >> 5) & 1
    eng = (flg >> 6) & 1
    vmc = ((flg >> 7) & 0xFF) * 4.0  # coarse bus pin-mV
    t_ms = np.arange(len(arr)) * DT_US / 1000.0
    valid = (step >= 1) & (step <= 6)
    name = TAGNAME.get(tag, tag)

    # -------- panel 1: VF & NEU vs time + accepts (the mechanism) --------
    ax = axes[bi][0]
    ax.plot(t_ms, vf, color="#1f77b4", lw=0.8, label="VF float")
    ax.plot(t_ms, neu, color="#d62728", lw=1.0, label="NEU (per-sector)")
    acc_t = t_ms[acc == 1]
    for x in acc_t:
        ax.axvline(x, color="#2ca02c", lw=0.5, alpha=0.35)
    ax.plot([], [], color="#2ca02c", lw=1, label=f"ZC accept ({acc.sum()})")
    ax.set_title(f"{name}  [ci={meta['ci']} dc={meta['dc']} vm={meta['vm']} kr={meta['kr']}]")
    ax.set_xlabel("time (ms)")
    ax.set_ylabel("mV")
    ax.legend(fontsize=7, loc="upper left")
    ax.grid(alpha=0.3)

    # -------- panel 2: coarse bus vs time (the surge/collapse) --------
    ax = axes[bi][1]
    ax.plot(t_ms, vmc, color="#9467bd", lw=1.0)
    ax.set_title("coarse bus (pin mV)  — collapse = desync surge")
    ax.set_xlabel("time (ms)")
    ax.set_ylabel("VM pin mV")
    ax.grid(alpha=0.3)
    # mark where accepts stop (last accept)
    if acc.any():
        last_acc_ms = t_ms[acc == 1][-1]
        ax.axvline(last_acc_ms, color="#2ca02c", ls="--", lw=1, label="last ZC accept")
        axes[bi][0].axvline(last_acc_ms, color="#2ca02c", ls="--", lw=1)
        ax.legend(fontsize=7)

    # -------- panel 3: equivalent-time BEMF shape (VF vs pos, per step) ----
    ax = axes[bi][2]
    m = valid & (pos_us < 20000)
    for s in range(1, 7):
        sm = m & (step == s)
        if sm.any():
            ax.scatter(pos_us[sm], vf[sm], s=5, color=step_colors[s - 1],
                       alpha=0.5, label=f"s{s}")
    if m.any():
        ax.axhline(np.median(neu[m]), color="k", ls="--", lw=1,
                   label=f"neu~{np.median(neu[m]):.0f}")
    ax.set_title("BEMF shape: VF vs position-in-window")
    ax.set_xlabel("us since commutation")
    ax.set_ylabel("VF mV")
    ax.legend(fontsize=6, ncol=4, loc="upper right")
    ax.grid(alpha=0.3)

fig.tight_layout()
out = capdir / f"{args.tag}.png"
fig.savefig(out, dpi=110)
print(f"saved plot -> {out}")

# --- text summary ---
print("\n=== per-block summary ===")
for tag, meta, arr in blocks:
    flg = arr[:, 3]
    step = flg & 0x7
    acc = (flg >> 5) & 1
    vmc = ((flg >> 7) & 0xFF) * 4.0
    valid = (step >= 1) & (step <= 6)
    dur_ms = len(arr) * DT_US / 1000.0
    name = TAGNAME.get(tag, tag)
    # accept cadence in the first vs last third (does it thin out?)
    third = len(arr) // 3
    a_first = acc[:third].sum()
    a_last = acc[-third:].sum()
    vm_first = vmc[:third][valid[:third]].mean() if valid[:third].any() else 0
    vm_last = vmc[-third:][valid[-third:]].mean() if valid[-third:].any() else 0
    print(f"  {name:18s} {dur_ms:5.0f}ms  accepts total={acc.sum():4d}  "
          f"first-third={a_first:3d} last-third={a_last:3d}  "
          f"bus {vm_first:.0f}->{vm_last:.0f} pin-mV")
