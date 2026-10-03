#!/usr/bin/env python3
"""Study section 3: ramps 10 -> 100 -> 10 % at four slews, from captures/char/ramp-*.txt.

Writes captures/char/fig/ramps.png and ramps.json.
  speed-vs-duty     recorder speed against the applied duty, up and down legs
  hysteresis        per 5 % duty bin (15..95 %), up-leg mean speed minus
                    down-leg mean speed; reported as the largest |difference|
                    and the value at 50 %
  lock events       the run's stop (reason), foldbacks, and dropout samples:
                    samples whose accepted crossings fell below half of what
                    the published average interval predicts for that sample
"""

from __future__ import annotations

import json
import pathlib
import statistics as st

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

import char_parse as cp  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
CAP = REPO / "captures" / "char"
FIG = CAP / "fig"
SLEWS = [("prod", "production staircase (1 %/500 ms)", "#1a7f4b"), ("200", "20 %/s", "#2a78d6"),
         ("2000", "200 %/s", "#e67e22"), ("step", "step", "#c0392b")]
NAMES = {
    2: "schedule end", 5: "phase current", 6: "bus floor", 8: "tracking", 13: "comparator storm",
    15: "late arm", 25: "average current", 26: "fast bus sag",
}


def legs(r):
    """Split recorder samples into the up leg (to the 100 % peak) and the down leg, closed loop only."""
    pts = [(t, d, e, a, q) for t, d, e, a, q in zip(r["t_s"], r["duty_pct"], r["ehz"], r["amps"], r["accepts"]) if t > 0]
    if not pts:
        return [], []
    peak = max(range(len(pts)), key=lambda i: pts[i][1])
    last_peak = max(i for i in range(len(pts)) if pts[i][1] >= pts[peak][1])
    return pts[: peak + 1], pts[last_peak:]


def main():
    FIG.mkdir(parents=True, exist_ok=True)
    fig, ax = plt.subplots(3, 1, figsize=(9, 10), dpi=105)
    rows = []
    for key, lab, col in SLEWS:
        p = CAP / f"ramp-{key}.txt"
        if not p.exists():
            continue
        d = cp.load(p)
        r = d["rec"]
        up, dn = legs(r)
        ax[0].plot([x[1] for x in up], [x[2] for x in up], "-", color=col, lw=1.1, label=f"{lab} up")
        ax[0].plot([x[1] for x in dn], [x[2] for x in dn], ":", color=col, lw=1.1, label=f"{lab} down")
        ax[1].plot([x[1] for x in up], [x[3] for x in up], "-", color=col, lw=0.9)
        ax[1].plot([x[1] for x in dn], [x[3] for x in dn], ":", color=col, lw=0.9)
        tt = [t for t in r["t_s"] if t > 0]
        span = max(tt) if tt else 1
        ax[2].plot([t / span for t in tt], [e for t, e in zip(r["t_s"], r["ehz"]) if t > 0], color=col, lw=0.9, label=lab)
        hyst = {}
        for b in range(15, 100, 5):
            u = [x[2] for x in up if b - 2.5 <= x[1] < b + 2.5]
            w = [x[2] for x in dn if b - 2.5 <= x[1] < b + 2.5]
            if u and w:
                hyst[b] = st.mean(u) - st.mean(w)
        drops = 0
        dt = None
        for i in range(1, len(r["t_s"])):
            if r["t_s"][i - 1] > 0:
                dt = r["t_s"][i] - r["t_s"][i - 1]
                avg = 1e6 / (6 * r["ehz"][i]) if r["ehz"][i] else None
                # The per-sample count is a byte: a sample whose expected count
                # exceeds 255 is saturated, not a dropout, and is not judged.
                exp = dt * 1e6 / avg if avg and dt > 0 else 0
                if avg and dt > 0 and 0.5 * exp < 255 and r["accepts"][i] < 0.5 * exp:
                    drops += 1
        cur = d["report"].get("BEMFCURRENT", {})
        rsn = cp.reason(d)
        rows.append(dict(file=p.name, slew=lab, reason=rsn, reason_name=NAMES.get(rsn, str(rsn)),
                         folds=cur.get("fold_slow", 0) + cur.get("fold_fast", 0), dropouts=drops,
                         peak_a=max(r["amps"]) if r else None,
                         hyst_max=max(hyst.values(), key=abs) if hyst else None, hyst_50=hyst.get(50),
                         reached=max(r["duty_pct"]) if r else None))
    ax[0].set_xlabel("applied duty, %")
    ax[0].set_ylabel("speed, eHz")
    ax[0].legend(fontsize=7, ncol=2)
    ax[1].set_xlabel("applied duty, %")
    ax[1].set_ylabel("current, A (metered)")
    ax[1].axhline(8.0, color="#555", lw=0.6, ls="--")
    ax[2].set_xlabel("time, fraction of each run's schedule")
    ax[2].set_ylabel("speed, eHz")
    ax[2].legend(fontsize=7)
    for a in ax:
        a.grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig(FIG / "ramps.png")
    json.dump({"rows": rows, "captures": [r["file"] for r in rows]}, open(FIG / "ramps.json", "w"), indent=1)
    print(f"ramps: {len(rows)} runs")


if __name__ == "__main__":
    main()
