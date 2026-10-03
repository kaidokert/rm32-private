#!/usr/bin/env python3
"""Study section 5: protections under real events, from every captures/char/*.txt.

Every run that stopped on anything but its schedule (reason != 2), or whose
governor folded the duty back, is an event. For each, the accept tail ring
(`CHARACC`, frozen at the first foldback or the stop) gives the last commutations:
the interval between successive accepted crossings, by sector, over the final
50 ms. Writes captures/char/fig/stops.png and stops.json.

Classification, from the stop reason and the trace:
  lost lock        tracking stop (8): accepted crossings stopped arriving
  current fold     slow/fast tracker crossed its limit (foldback, or stop 25)
  bus              floor or fast sag (6, 26)
  timing           late arm (15), handler overrun (14), comparator storm (13)
The trace adds whether the intervals were steady up to the event (a step or
current limit) or stretched/scattered first (a lock loss building up).
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
SECT = ["#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b"]
CLASS = {8: "lost lock", 25: "current (stop)", 6: "bus floor", 26: "fast bus sag", 15: "timing: late arm",
         14: "timing: handler overrun", 13: "timing: comparator storm", 5: "phase current"}
WINDOW_US = 50_000


def trace(acc):
    """(t_us relative to the last crossing, interval_us, sector, late) over the ring, unwrapped."""
    if len(acc) < 3:
        return []
    t = [0]
    for (a, *_), (b, *_) in zip(acc, acc[1:]):
        t.append(t[-1] + ((b - a) & 0xFFFF))
    end = t[-1]
    out = []
    for i in range(1, len(acc)):
        out.append((t[i] - end, t[i] - t[i - 1], acc[i][2], acc[i][4]))
    return [x for x in out if x[0] >= -WINDOW_US]


def _grp(name: str) -> str:
    """Experiment name without the run number (step-A-B-k, restart-D-k only)."""
    stem = name[:-4]
    if stem.startswith(("step-", "restart-")):
        return stem.rsplit("-", 1)[0]
    return stem


def main():
    FIG.mkdir(parents=True, exist_ok=True)
    events = []
    for p in sorted(CAP.glob("*.txt")):
        if p.name.startswith(("smoke", "campaign")):
            continue
        d = cp.load(p)
        rsn = cp.reason(d)
        cur = d["report"].get("BEMFCURRENT", {})
        folds = cur.get("fold_slow", 0) + cur.get("fold_fast", 0)
        if rsn in (None, 2) and not folds:
            continue
        tr = trace(d["acc"])
        ivs = [x[1] for x in tr]
        steady = None
        if len(ivs) > 24:
            head = st.median(ivs[: len(ivs) // 2])
            tail = ivs[-12:]
            steady = max(tail) < 1.5 * head and min(tail) > 0.5 * head
        cls = CLASS.get(rsn, f"reason {rsn}") if rsn != 2 else "current fold (no stop)"
        events.append(dict(file=p.name, reason=rsn, cls=cls, folds=folds, fold_slow=cur.get("fold_slow", 0),
                           fold_fast=cur.get("fold_fast", 0), n=len(tr), late=sum(1 for x in tr if x[3]),
                           median_iv=st.median(ivs) if ivs else None, max_iv=max(ivs) if ivs else None,
                           steady=steady, tr=tr))
    # One panel per (experiment, class) group -- the first event of the group,
    # titled with how many events it stands for; the table lists every event.
    import re as _re
    groups = {}
    for e in events:
        g = (_grp(e["file"]), e["cls"])
        groups.setdefault(g, []).append(e)
    for e in events:
        e["group"] = _grp(e["file"])
    reps = [(g, v[0], len(v)) for g, v in groups.items()]
    n = len(reps)
    if n:
        fig, ax = plt.subplots(n, 1, figsize=(9, 1.75 * n + 0.6), dpi=100, squeeze=False)
        for i, (g, e, cnt) in enumerate(reps):
            a = ax[i][0]
            for sct in range(1, 7):
                pts = [(x[0] / 1000, x[1]) for x in e["tr"] if x[2] == sct]
                a.plot([q[0] for q in pts], [q[1] for q in pts], ".", ms=2.5, color=SECT[sct - 1])
            lt = [(x[0] / 1000, x[1]) for x in e["tr"] if x[3]]
            if lt:
                a.plot([q[0] for q in lt], [q[1] for q in lt], "x", color="k", ms=5)
            if not e["tr"]:
                a.text(-25, 0.5, "ring empty: the stop came within a few commutations of loop close", ha="center", fontsize=8, transform=a.get_xaxis_transform())
            a.set_ylabel("interval, µs", fontsize=8)
            a.set_title(f"{g[0]} — {g[1]} ({cnt} event{'s' if cnt > 1 else ''}; trace: {e['file']})", fontsize=9, loc="left")
            a.set_xlim(-WINDOW_US / 1000, 0.5)
            a.grid(alpha=0.3)
        ax[-1][0].set_xlabel("ms before the ring froze (first foldback or the stop); colour = sector, x = late arm")
        fig.tight_layout()
        fig.savefig(FIG / "stops.png")
    for e in events:
        e.pop("tr")
    json.dump({"events": events, "captures": [e["file"] for e in events]}, open(FIG / "stops.json", "w"), indent=1)
    print(f"stops: {len(events)} events in {n} groups")


if __name__ == "__main__":
    main()
