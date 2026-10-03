#!/usr/bin/env python3
"""Study section 4: the low end, from captures/char/low-*.txt, map-*.txt and restart-*.txt.

Writes captures/char/fig/lowend.png and lowend.json.
  lowest held duty  the lowest duty whose low-DDD run (production staircase down
                    from the 10 % close, then a 30 s hold) ended on its schedule
                    (reason 2); every low-* run is listed with its stop
  restart time      bridge on -> 90 % of the final speed: the firmware's startup
                    (CHARSNAP startup_us, bridge on to loop closed) plus the
                    time after close at which the recorder's speed first
                    reached 90 % of its mean over the last 0.5 s
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
COL = {100: "#1a7f4b", 500: "#2a78d6", 1000: "#c0392b"}


def main():
    FIG.mkdir(parents=True, exist_ok=True)
    low = []
    for p in sorted(CAP.glob("low-*.txt")):
        d = cp.load(p)
        duty = int(re.match(r"low-(\d+)", p.stem).group(1)) / 10
        # The firmware's hold mark needs duty >= the run target (the 10 % close
        # duty for these profiles), so below 10 % it never fires. Held = the
        # schedule ran to its end (reason 2); speed and current are the
        # recorder's means over the last 25 s, i.e. inside the 30 s hold.
        r = d["rec"]
        tend = max(r["t_s"]) if r else 0
        sel = [i for i, t in enumerate(r["t_s"]) if t > 0 and t >= tend - 25] if r else []
        rsn = cp.reason(d)
        low.append(dict(file=p.name, duty=duty, reason=rsn, held=rsn == 2,
                        ran_s=tend,
                        ehz=st.mean(r["ehz"][i] for i in sel) if sel and rsn == 2 else None,
                        hold_a=st.mean(r["amps"][i] for i in sel) if sel and rsn == 2 else None))
    held = [r["duty"] for r in low if r["held"]]
    rst = []
    fig, ax = plt.subplots(1, 1, figsize=(9, 4.2), dpi=105)
    for p in sorted(CAP.glob("restart-*.txt")):
        d = cp.load(p)
        m = re.match(r"restart-(\d+)-(\d+)", p.stem)
        duty, k = int(m.group(1)), int(m.group(2))
        r = d["rec"]
        su = d["snap"].get("startup_us", 0) / 1e6
        pts = [(t, e) for t, e in zip(r["t_s"], r["ehz"]) if t > 0]
        res = dict(file=p.name, duty=duty / 10, k=k, reason=cp.reason(d), startup_s=su)
        if pts and res["reason"] != 2:
            res.update(stop_after_close_s=pts[-1][0], ehz_at_stop=pts[-1][1])
        elif pts:
            tend = pts[-1][0]
            final = st.mean([e for t, e in pts if t >= tend - 0.5])
            t90 = next((t for t, e in pts if e >= 0.9 * final), None)
            res.update(final_ehz=final, t90_close_s=t90, restart_s=su + t90 if t90 is not None else None)
            ax.plot([su + t for t, _ in pts], [e for _, e in pts], color=COL.get(duty, "#555"), lw=0.9,
                    label=f"{duty / 10:g} %" if k == 1 else None)
        rst.append(res)
    ax.set_xlabel("time from bridge on, s (startup + closed loop)")
    ax.set_ylabel("speed, eHz")
    ax.legend(fontsize=8)
    ax.grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig(FIG / "lowend.png")
    json.dump({"low": low, "lowest_held": min(held) if held else None, "restart": rst,
               "captures": [r["file"] for r in low] + [r["file"] for r in rst]}, open(FIG / "lowend.json", "w"), indent=1)
    print(f"lowend: {len(low)} low runs (lowest held {min(held) if held else None}), {len(rst)} restarts")


if __name__ == "__main__":
    main()
