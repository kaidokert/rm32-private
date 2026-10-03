#!/usr/bin/env python3
"""Study section 2: step response from captures/char/step-A-B-k.txt.

Writes captures/char/fig/steps.png and steps.json. The step instant is known
from the profile: the run reaches A at 100 tenths/s from the 10 % close, holds
4 s, then B is commanded in one step (`charz` schedule), so
t_step = |A - 100| * 10 ms + 4 s after the loop closed.

Per run, from the recorder (3.33 ms samples):
  before / after   mean speed over [-0.4, 0] s and [+1.2, +1.5] s of the step
  t90              first sample at which speed has covered 90 % of the change
  overshoot        beyond the final value, % of the change (0 if none)
  peak current     max metered current in [0, +1.5] s
  worst bus dip    min bus in [0, +1.5] s minus the pre-step bus mean, V
  lock held        the run ended on its schedule (reason 2) and the
                   governor never folded the duty back
"""

from __future__ import annotations

import json
import pathlib
import re
import statistics as st

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

import char_parse as cp  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
CAP = REPO / "captures" / "char"
FIG = CAP / "fig"
ORDER = [(200, 500), (200, 800), (500, 1000), (100, 1000), (1000, 500), (800, 200), (1000, 100)]
RUNC = ["#2a78d6", "#e67e22", "#1a7f4b"]


def window(r, t0, a, b, key):
    return [v for t, v in zip(r["t_s"], r[key]) if t0 + a <= t <= t0 + b]


def analyse(p):
    d = cp.load(p)
    m = re.match(r"step-(\d+)-(\d+)-(\d+)", p.stem)
    a, b, k = (int(x) for x in m.groups())
    r = d["rec"]
    t_step = abs(a - 100) * 0.010 + 4.0
    res = dict(file=p.name, a=a, b=b, k=k, reason=cp.reason(d),
               ceiling=d["report"].get("BEMFCURRENT", {}).get("ceiling_tenths", 0),
               fold=d["report"].get("BEMFCURRENT", {}).get("fold_slow", 0) + d["report"].get("BEMFCURRENT", {}).get("fold_fast", 0))
    if not r or not window(r, t_step, -0.4, 0, "ehz"):
        res.update(ok=False)
        return res, None
    # Metrics on a 5-sample (~17 ms) centred moving mean of speed: one sample
    # is one published interval and carries ~±60 eHz of sector-to-sector
    # asymmetry, which would otherwise read as overshoot.
    e = r["ehz"]
    r = dict(r)
    r["ehz_s"] = [st.mean(e[max(0, i - 2): i + 3]) for i in range(len(e))]
    before = st.mean(window(r, t_step, -0.4, 0, "ehz"))
    aft = window(r, t_step, 1.2, 1.5, "ehz")
    after = st.mean(aft) if aft else None
    t90 = over = None
    if after is not None and abs(after - before) > 1:
        ch = after - before
        target = before + 0.9 * ch
        for t, v in zip(r["t_s"], r["ehz_s"]):
            if t >= t_step and ((ch > 0 and v >= target) or (ch < 0 and v <= target)):
                t90 = t - t_step
                break
        post = window(r, t_step, 0, 1.5, "ehz_s")
        ext = max(post) if ch > 0 else min(post)
        over = max(0.0, (ext - after) / ch * 100) if ch > 0 else max(0.0, (after - ext) / -ch * 100)
    bus0 = st.mean(window(r, t_step, -0.4, 0, "bus_v"))
    post_t = [t for t in r["t_s"] if t >= t_step]
    res.update(stop_ms=(max(post_t) - t_step) * 1000 if post_t and res["reason"] != 2 else None,
               ehz_reached=max(window(r, t_step, 0, 1.5, "ehz_s") or [before]))
    res.update(ok=True, before=before, after=after, t90_ms=t90 * 1000 if t90 is not None else None, over_pct=over,
               peak_a=max(window(r, t_step, 0, 1.5, "amps")) if window(r, t_step, 0, 1.5, "amps") else None,
               dip_v=(min(window(r, t_step, 0, 1.5, "bus_v")) - bus0) if window(r, t_step, 0, 1.5, "bus_v") else None,
               held=res["reason"] == 2 and res["fold"] == 0)
    return res, (r, t_step)


def main():
    FIG.mkdir(parents=True, exist_ok=True)
    allres = []
    fig, ax = plt.subplots(len(ORDER), 2, figsize=(11, 2.2 * len(ORDER)), dpi=100, sharex=True)
    for i, (a, b) in enumerate(ORDER):
        for p in sorted(CAP.glob(f"step-{a:03d}-{b:03d}-*.txt")):
            res, tr = analyse(p)
            allres.append(res)
            if tr is None:
                continue
            r, t0 = tr
            ts = [t - t0 for t in r["t_s"]]
            c = RUNC[(res["k"] - 1) % 3]
            ax[i][0].plot(ts, r["ehz"], color=c, lw=0.9, label=f"run {res['k']}" + ("" if res["held"] else " (stop/fold)"))
            ax[i][1].plot(ts, r["amps"], color=c, lw=0.8)
        ax[i][0].set_ylabel(f"{a / 10:g}→{b / 10:g} %\neHz", fontsize=9)
        ax[i][1].set_ylabel("A", fontsize=9)
        ax[i][0].axvline(0, color="#999", lw=0.7)
        ax[i][1].axvline(0, color="#999", lw=0.7)
        ax[i][1].axhline(8.0, color="#555", lw=0.6, ls="--")
        if ax[i][0].get_legend_handles_labels()[0]:
            ax[i][0].legend(fontsize=7, loc="best")
        for j in (0, 1):
            ax[i][j].grid(alpha=0.3)
            ax[i][j].set_xlim(-0.5, 1.5)
    ax[0][0].set_title("speed (from the average commutation interval)", fontsize=10)
    ax[0][1].set_title("current (metered scale)", fontsize=10)
    ax[-1][0].set_xlabel("time from the step, s")
    ax[-1][1].set_xlabel("time from the step, s")
    fig.tight_layout()
    fig.savefig(FIG / "steps.png")
    json.dump({"rows": allres, "captures": [r["file"] for r in allres]}, open(FIG / "steps.json", "w"), indent=1)
    print(f"steps: {len(allres)} runs")


if __name__ == "__main__":
    main()
