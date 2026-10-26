"""A/B comparison of binz R3 (10 -> 60 % slam) runs between two images.

Per run: kill flag, peak guard EWMA (recorder `ma`, the quantity OCFAST
compares to 4800 mA) over the whole run and in the slam window (first
0.5 s after the 10 -> 60 % step), mean `ma` over the 60 % dwell, and the
r-line `i50max` / `imean_ma`. Per metric: mean, sd, Welch t and its
two-sided p (normal approximation to t when scipy is missing).

    python scripts/ab_r3_peaks.py <tagA> <tagB>   (e.g. bis51A bis51B)

Refuses (exit 2) when either side has no runs or a run lacks a field.
"""

import csv
import json
import math
import pathlib
import re
import sys

STEP_T = 3.0   # binz r3 profile: 10:3 then 60:4
SLAM_W = 0.5
DWELL = (STEP_T + 1.0, STEP_T + 4.0)


def runs(tag):
    out = []
    for js in sorted(pathlib.Path("captures/binz").glob(f"profile_{tag}-r3-step_*.json")):
        d = json.loads(js.read_text())
        rows = list(csv.DictReader(js.with_suffix(".csv").open()))
        t = [float(r["t_s"]) for r in rows]
        ma = [int(r["ma"]) for r in rows]
        slam = [m for ti, m in zip(t, ma) if STEP_T <= ti < STEP_T + SLAM_W]
        dwell = [m for ti, m in zip(t, ma) if DWELL[0] <= ti < DWELL[1]]
        rline = d["r"] if isinstance(d["r"], str) else " ".join(f"{k}={v}" for k, v in d["r"].items())
        kv = dict(re.findall(r"(\w+)=(-?\d+)", rline))
        if not slam or not dwell or "i50max" not in kv or "imean_ma" not in kv:
            sys.exit(f"refuse: {js.name} lacks a field (slam {len(slam)}, dwell {len(dwell)}, r={rline!r})")
        out.append({
            "run": js.stem.split("_", 2)[-1],
            "kill": bool(d.get("kill")),
            "peak_ma": max(ma),
            "slam_ma": max(slam),
            "dwell_ma": sum(dwell) / len(dwell),
            "i50max": int(kv["i50max"]),
            "imean": int(kv["imean_ma"]),
        })
    return out


def welch(a, b):
    ma, mb = sum(a) / len(a), sum(b) / len(b)
    va = sum((x - ma) ** 2 for x in a) / (len(a) - 1)
    vb = sum((x - mb) ** 2 for x in b) / (len(b) - 1)
    se = math.sqrt(va / len(a) + vb / len(b))
    if se == 0:
        return ma, mb, math.sqrt(va), math.sqrt(vb), 0.0, 1.0
    t = (mb - ma) / se
    df = (va / len(a) + vb / len(b)) ** 2 / (
        (va / len(a)) ** 2 / (len(a) - 1) + (vb / len(b)) ** 2 / (len(b) - 1))
    try:
        from scipy.stats import t as tdist
        p = 2 * tdist.sf(abs(t), df)
    except ImportError:
        p = math.erfc(abs(t) / math.sqrt(2))
    return ma, mb, math.sqrt(va), math.sqrt(vb), t, p


def main():
    a_tag, b_tag = sys.argv[1], sys.argv[2]
    a, b = runs(a_tag), runs(b_tag)
    if not a or not b:
        print(f"refuse: {a_tag} {len(a)} runs, {b_tag} {len(b)} runs")
        return 2
    for tag, rs in ((a_tag, a), (b_tag, b)):
        for r in rs:
            print(f"{tag} {r['run']} kill={int(r['kill'])} peak_ma={r['peak_ma']} "
                  f"slam_ma={r['slam_ma']} dwell_ma={r['dwell_ma']:.0f} "
                  f"i50max={r['i50max']} imean={r['imean']}")
    print(f"kills: {a_tag} {sum(r['kill'] for r in a)}/{len(a)}, "
          f"{b_tag} {sum(r['kill'] for r in b)}/{len(b)}")
    for k in ("peak_ma", "slam_ma", "dwell_ma", "i50max", "imean"):
        ma, mb, sa, sb, t, p = welch([r[k] for r in a], [r[k] for r in b])
        print(f"{k:9s} {a_tag} {ma:7.1f} sd {sa:5.1f} | {b_tag} {mb:7.1f} sd {sb:5.1f} "
              f"| diff {mb - ma:+7.1f}  t={t:+.2f} p={p:.4f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
