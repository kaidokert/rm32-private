"""A/B comparison of every banded metric (settled duty / eHz / mA per
segment, gc_summary.py) between two run tags, e.g. an interleaved
step-vs-previous-step proof.

    python scripts/ab_segments.py <tagA> <tagB> [--alpha 0.05]

Per (board, round, segment, quantity): mean of each side, the difference,
the difference as a fraction of the S00 band width, and Welch's t / p.
Rounds with no S00 band (the battery runs, gc_battery.sh) compare every
segment of at least --min-secs, with no band fraction.
Flags a metric when p < alpha / (number of metrics compared) (Bonferroni),
and separately lists every raw p < alpha so nothing is hidden. Refuses
(exit 2) when a side has no runs, a killed run, or the two sides disagree
on round/segment layout.
"""

import argparse
import json
import math
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
from gc_summary import summarize  # noqa: E402


def welch(a, b):
    ma, mb = sum(a) / len(a), sum(b) / len(b)
    va = sum((x - ma) ** 2 for x in a) / (len(a) - 1)
    vb = sum((x - mb) ** 2 for x in b) / (len(b) - 1)
    se = math.sqrt(va / len(a) + vb / len(b))
    if se == 0:
        return ma, mb, 0.0, 1.0 if ma == mb else 0.0
    t = (mb - ma) / se
    df = (va / len(a) + vb / len(b)) ** 2 / (
        (va / len(a)) ** 2 / (len(a) - 1) + (vb / len(b)) ** 2 / (len(b) - 1))
    try:
        from scipy.stats import t as tdist
        p = 2 * tdist.sf(abs(t), df)
    except ImportError:
        sys.exit("refuse: scipy missing (normal approximation overstates significance at n=3)")
    return ma, mb, t, p


def collect(tag):
    out = {}
    for board in ("l431", "binz"):
        for js in sorted(pathlib.Path(f"captures/{board}").glob(f"profile_{tag}-r*.json")):
            s = summarize(js, 2.0)
            if s["kill"]:
                sys.exit(f"refuse: {js.name} was killed ({s['kill']})")
            rnd = s["test"].split("-")[1]
            out.setdefault((board, rnd), []).append(s)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--alpha", type=float, default=0.05)
    ap.add_argument("--bands", default="../docs/gc_bands_s00.json")
    ap.add_argument("--min-secs", type=float, default=2.0,
                    help="unbanded rounds: shortest segment compared")
    a = ap.parse_args()
    bands = json.loads(pathlib.Path(a.bands).read_text())
    A, B = collect(a.a), collect(a.b)
    if not A or not B or A.keys() != B.keys():
        print(f"refuse: layouts differ {sorted(A)} vs {sorted(B)}")
        return 2
    rows = []
    for key in sorted(A):
        board, rnd = key
        band = next((v for k, v in bands.items() if k.startswith(f"{board}/s00-{rnd}-")), None)
        nseg = {len(s["segments"]) for s in A[key] + B[key]}
        if len(nseg) != 1:
            print(f"refuse: {key} segment counts {nseg}")
            return 2
        for i in range(nseg.pop()):
            for q in ("duty", "ehz", "ma"):
                if band is None:
                    if A[key][0]["segments"][i]["secs"] < a.min_secs:
                        continue
                    bq = None
                else:
                    bq = band["segments"][i][q]
                    if bq is None:
                        continue
                xa = [s["segments"][i][q] for s in A[key]]
                xb = [s["segments"][i][q] for s in B[key]]
                ma, mb, t, p = welch(xa, xb)
                frac = (mb - ma) / (bq[1] - bq[0]) if bq and bq[1] > bq[0] else float("nan")
                thr = A[key][0]["segments"][i]["throttle"]
                rows.append((board, rnd, i, thr, q, ma, mb, frac, t, p, len(xa), len(xb)))
    bonf = a.alpha / len(rows)
    print(f"{len(rows)} metrics; Bonferroni threshold p < {bonf:.5f}")
    for r in rows:
        board, rnd, i, thr, q, ma, mb, frac, t, p, na, nb = r
        mark = "SIGNIFICANT" if p < bonf else ("raw-p" if p < a.alpha else "")
        print(f"{board:4} {rnd} seg{i} {thr:3d}% {q:4} {ma:8.1f} -> {mb:8.1f} "
              f"({mb - ma:+7.1f}, {frac:+.0%} of band) t={t:+6.2f} p={p:.4f} n={na}/{nb} {mark}")
    sig = [r for r in rows if r[9] < bonf]
    print(f"significant after Bonferroni: {len(sig)}; raw p < {a.alpha}: "
          f"{sum(r[9] < a.alpha for r in rows)} (expected by chance ~{a.alpha * len(rows):.1f})")
    return 1 if sig else 0


if __name__ == "__main__":
    sys.exit(main())
