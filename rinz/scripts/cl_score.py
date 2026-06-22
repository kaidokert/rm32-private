#!/usr/bin/env python3
"""Score the Rust cl_replay trace against the oracle — the Stage-1b gate (G1/G2).

Reads `logs/cl_trace.txt` (cols: cap comm phys zc_raw zc_track accepted oracle), where
zc_track is the firmware tracker's FILTERED estimate and oracle is the offline
multi-harmonic truth (carried through from cl_export.py). Reports:

  G1 tracking : median |zc_track - oracle| vs the raw |zc_raw - oracle|, in-window.
  G2 rejection: spread (commutation-to-commutation) of zc_track vs zc_raw -- the
                filter should be much smoother AND land on the oracle, not an artifact.

This is oracle-anchored (CLOSED_LOOP_PLAN.md Stage 1b): we gate on the FILTERED
estimate control would use, not raw crossings. G1 threshold default 5%.

    python scripts/cl_score.py logs/cl_trace.txt --render
"""

from __future__ import annotations

import argparse
import statistics
from collections import defaultdict
from pathlib import Path


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("trace", type=Path, nargs="?", default=Path("logs/cl_trace.txt"))
    ap.add_argument("--g1", type=float, default=5.0)
    ap.add_argument("--render", action="store_true")
    args = ap.parse_args()

    rows = []
    for ln in args.trace.read_text().splitlines():
        if ln.startswith("#") or not ln.strip():
            continue
        cap, comm, phys, zc_raw, zc_track, acc, oracle = ln.split()
        rows.append({
            "cap": int(cap), "comm": int(comm), "phys": int(phys),
            "zc_raw": float(zc_raw), "zc_track": float(zc_track),
            "acc": int(acc), "oracle": float(oracle),
        })
    if not rows:
        raise SystemExit("empty trace -- run cl_export.py then `cargo cl-replay`")

    inwin = [r for r in rows if r["oracle"] >= 0]
    track_err = [abs(r["zc_track"] - r["oracle"]) for r in inwin]
    raw_err = [abs(r["zc_raw"] - r["oracle"]) for r in inwin if r["zc_raw"] >= 0]
    n_comm = len(rows)
    n_raw_inwin = sum(1 for r in rows if r["zc_raw"] >= 0)
    n_accepted = sum(r["acc"] for r in rows)

    print(f"{n_comm} commutations over {len({r['cap'] for r in rows})} captures; "
          f"{n_raw_inwin} raw in-window, {n_accepted} accepted by the gate, "
          f"{len(inwin)} with oracle truth")

    print("\n=== G1  tracking vs oracle (1% window = 0.6 deg) ===")
    if raw_err:
        print(f"  RAW   |zc_raw   - oracle|: median {statistics.median(raw_err):5.1f}%  "
              f"(n={len(raw_err)})")
    if track_err:
        tm = statistics.median(track_err)
        print(f"  TRACK |zc_track - oracle|: median {tm:5.1f}%  max {max(track_err):5.1f}%  "
              f"<{args.g1:.0f}%: {sum(1 for x in track_err if x < args.g1)}/{len(track_err)}")

    # G2: commutation-to-commutation spread, per capture (filtered should be smoother)
    def jitter(key):
        js = []
        bycap = defaultdict(list)
        for r in rows:
            bycap[r["cap"]].append(r)
        for v in bycap.values():
            seq = [r[key] for r in v if (key != "zc_raw" or r["zc_raw"] >= 0)]
            for i in range(1, len(seq)):
                js.append(abs(seq[i] - seq[i - 1]))
        return statistics.median(js) if js else float("nan")

    print("\n=== G2  outlier rejection (commutation-to-commutation jitter) ===")
    print(f"  RAW   step-to-step jitter: {jitter('zc_raw'):5.1f}%")
    print(f"  TRACK step-to-step jitter: {jitter('zc_track'):5.1f}%  (should be << raw)")

    print("\n=== STAGE-1b GATE ===")
    if track_err:
        tm = statistics.median(track_err)
        rj, tj = jitter("zc_raw"), jitter("zc_track")
        g1 = tm <= args.g1
        g2 = tj < rj * 0.6
        print(f"  G1 (track median <= {args.g1:.0f}%): {g1}  ({tm:.1f}%)")
        print(f"  G2 (track jitter << raw):     {g2}  ({tj:.1f}% vs {rj:.1f}%)")
        if g1 and g2:
            print("  -> PASS: the filtered estimate tracks the oracle and rejects the raw noise.")
        else:
            print("  -> not yet. Tune CL_KP/CL_KI/CL_GATE/CL_BLANK and re-run `cargo cl-replay`.")

    if args.render:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        caps = sorted({r["cap"] for r in rows})[:6]
        fig, axes = plt.subplots(len(caps), 1, figsize=(11, 2.0 * len(caps)), squeeze=False)
        for ax, cp in zip(axes[:, 0], caps):
            cr = [r for r in rows if r["cap"] == cp]
            x = [r["comm"] for r in cr]
            ax.plot(x, [r["zc_raw"] if r["zc_raw"] >= 0 else None for r in cr],
                    "x", color="tab:red", alpha=0.6, label="raw")
            ax.plot(x, [r["zc_track"] for r in cr], "-", color="tab:blue", label="tracked")
            ax.plot(x, [r["oracle"] if r["oracle"] >= 0 else None for r in cr],
                    "o", color="green", ms=4, label="oracle")
            ax.set_ylim(0, 100)
            ax.set_ylabel(f"cap {cp}\nZC %", fontsize=8)
            ax.grid(alpha=0.3)
        axes[0, 0].legend(fontsize=8, ncol=3)
        axes[-1, 0].set_xlabel("commutation")
        fig.suptitle("Stage-1b: raw detector vs filtered tracker vs oracle")
        fig.tight_layout()
        out = args.trace.parent / "cl_track.png"
        fig.savefig(out, dpi=110)
        print(f"\nwrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
