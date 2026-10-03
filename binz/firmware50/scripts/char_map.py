#!/usr/bin/env python3
"""Study section 1: operating map 5-100 % from captures/char/map-*.txt.

Writes captures/char/fig/map.png and map.json (table rows + capture list).
Per rung, after settle (the window charz counts in: 3 s after reaching the duty
to the end of the 10 s hold):
  speed       closed loop: BEMFRATE ehz_from_sector (hold mean); coast: the
              time-anchored coast estimate (cohort.coast_ehz) after the stop
  current     BEMFCURRENT hold_ma (hold mean) and ews_hold_max_ma (slow-tracker peak)
  bus         BEMFSAG filt_bus at the end of the run, in volts
  jitter      p50/p99 of |interval - mean of last six| per sector (CHARHIST)
  rates       late commutations (service >= LATE_US late), excursions and late
              arms, per 1k commutations in the window
"""

from __future__ import annotations

import json
import pathlib
import re

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

import char_parse as cp  # noqa: E402
import cohort  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
CAP = REPO / "captures" / "char"
FIG = CAP / "fig"
SECT = ["#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b"]


def rows():
    out = []
    for p in sorted(CAP.glob("map-*.txt")):
        d = cp.load(p)
        duty = int(re.match(r"map-(\d+)", p.stem).group(1)) / 10
        rep = d["report"]
        rsn = cp.reason(d)
        text = p.read_text(encoding="utf-8", errors="replace")
        m = re.search(r"COASTTIMING .*?iv_us=([\d,]+)", text)
        coast = cohort.coast_ehz([int(x) for x in m.group(1).split(",") if x]) if m else 0
        cur = rep.get("BEMFCURRENT", {})
        # Held = the run ended on its schedule. (Below the 10 % close duty the hold
        # mark is set at close, so hold_ms alone does not mean the rung held.)
        held = rsn == 2 and rep.get("BEMFRATE", {}).get("hold_ms", 0) > 0
        out.append(dict(
            duty=duty, file=p.name, reason=rsn,
            ceiling=cur.get("ceiling_tenths", 0) / 10,
            loop_ehz=rep.get("BEMFRATE", {}).get("ehz_from_sector", 0) if held else None,
            coast_ehz=coast or None,
            hold_a=cur.get("hold_ma", 0) / 1000 if held else None,
            slow_a=cur.get("ews_hold_max_ma", 0) / 1000 if held else None,
            bus_v=cp.bus_volts(d, rep["BEMFSAG"]["filt_bus"]) if "BEMFSAG" in rep else None,
            p50=[cp.pct(h, 0.5) for h in d["hist"]],
            p99=[cp.pct(h, 0.99) for h in d["hist"]],
            late_1k=cp.late_per_1k(d),
            exc_1k=cp.per_1k(d, "excursions"),
            arm_1k=cp.per_1k(d, "late_arms"),
            n=d["count"].get("accepts", 0),
        ))
    return sorted(out, key=lambda r: r["duty"])


def main():
    FIG.mkdir(parents=True, exist_ok=True)
    rs = rows()
    held = [r for r in rs if r["loop_ehz"]]
    fig, ax = plt.subplots(4, 1, figsize=(9, 11.5), sharex=True, dpi=110)
    x = [r["duty"] for r in held]
    ax[0].plot(x, [r["loop_ehz"] for r in held], "o-", color="#2a78d6", label="closed loop (hold mean)")
    ax[0].plot([r["duty"] for r in held if r["coast_ehz"]], [r["coast_ehz"] for r in held if r["coast_ehz"]],
               "s--", color="#7a8a9e", ms=4, label="coast after stop")
    ax[0].set_ylabel("speed, eHz")
    ax[1].plot(x, [r["hold_a"] for r in held], "o-", color="#c0392b", label="hold mean")
    ax[1].plot(x, [r["slow_a"] for r in held], "^:", color="#e67e22", label="slow-tracker peak (207 ms)")
    ax[1].axhline(8.0, color="#555", lw=0.8, ls="--")
    ax[1].text(55, 7.75, "8 A allowance (slow tracker)", fontsize=8, color="#555", va="top")
    ax[1].set_ylabel("current, A (metered)")
    ax[2].plot(x, [r["bus_v"] for r in held], "o-", color="#1a7f4b")
    ax[2].set_ylabel("bus at hold, V")
    for s in range(6):
        ax[3].plot(x, [r["p50"][s] for r in held], "-", color=SECT[s], lw=1.2, label=f"s{s + 1}")
        ax[3].plot(x, [r["p99"][s] for r in held], "--", color=SECT[s], lw=0.9)
    ax[3].set_ylabel("|Δinterval|, µs\n(solid p50, dashed p99)")
    ax[3].set_xlabel("duty, %")
    for a in ax:
        a.grid(alpha=0.3)
    ax[0].legend(fontsize=8, loc="upper left")
    ax[1].legend(fontsize=8, loc="upper left")
    ax[3].legend(fontsize=7, ncol=6, loc="upper left")
    for r in rs:
        if r["reason"] != 2 or r["ceiling"] < r["duty"]:
            for a in ax:
                a.axvline(r["duty"], color="#c0392b", alpha=0.25, lw=6)
    fig.tight_layout()
    fig.savefig(FIG / "map.png")
    json.dump({"rows": rs, "captures": [r["file"] for r in rs]}, open(FIG / "map.json", "w"), indent=1)
    print(f"map: {len(rs)} rungs, {len(held)} held")


if __name__ == "__main__":
    main()
