#!/usr/bin/env python3
"""Plot the closed-loop simulation trajectory (logs/cl_sim.txt from `cargo cl-sim`).

Shows the loop tracking the rotor through the load step: rotor speed, the filtered
period estimate vs the true period, and where the loop coasted (missed-ZC dead
reckoning). The two should ride together if the loop is genuinely closing on the ZC.

    python scripts/cl_sim_plot.py [logs/cl_sim.txt]
"""

from __future__ import annotations

import sys
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("logs/cl_sim.txt")
rows = [ln.split() for ln in path.read_text().splitlines() if ln and not ln.startswith("#")]
tick = [int(r[0]) for r in rows]
omega = [float(r[2]) for r in rows]
period_est = [float(r[4]) for r in rows]
true_period = [float(r[5]) for r in rows]
coasted = [int(r[8]) for r in rows]
commutate = [int(r[6]) for r in rows]

fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(11, 6), sharex=True)
ax1.plot(tick, omega, color="tab:green")
ax1.set_ylabel("rotor omega\n(deg/tick)")
ax1.grid(alpha=0.3)
ax1.set_title("Closed-loop sim: loop tracks the rotor through a load step (ZC feedback)")

ax2.plot(tick, true_period, color="black", lw=1.5, label="true period (rotor)")
ax2.plot(tick, period_est, color="tab:blue", lw=1.0, label="period_est (loop)")
# mark coasted commutations (missed ZC -> dead reckon)
cx = [t for t, c, co in zip(tick, commutate, coasted) if c and co]
cy = [p for p, c, co in zip(period_est, commutate, coasted) if c and co]
ax2.plot(cx, cy, "x", color="tab:red", ms=4, alpha=0.5, label="coasted commutation")
ax2.set_ylabel("sector period\n(ticks)")
ax2.set_xlabel("tick")
ax2.legend(fontsize=8, loc="upper left")
ax2.grid(alpha=0.3)

fig.tight_layout()
out = path.parent / "cl_sim.png"
fig.savefig(out, dpi=110)
print(f"wrote {out}")
