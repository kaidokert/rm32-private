#!/usr/bin/env python3
"""Offline analysis of a scope_sweep.py run.

Parses logs/sweep_<ts>/, computes per-capture metrics, builds (freq, amp)
heatmaps, and flags the interesting points -- the thin transition bands and the
marginal/bistable zone (where repeated snapshots at the *same* setpoint disagree,
i.e. your "a tiny amp change flips lock" effect, quantified as snapshot spread).
Optionally batch-renders the ZC plot for the flagged points only.

    python scripts/sweep_map.py logs/sweep_20260614_xxxx
    python scripts/sweep_map.py logs/sweep_20260614_xxxx --render        # images for flagged pts
    python scripts/sweep_map.py logs/sweep_20260614_xxxx --top 40

We do NOT assert a binary locked/unlocked (we proved that's unreliable). We map
the observable metrics; the lock structure shows up as structure in the maps.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parent))
import numpy as np

from scope_common import (
    analyze_zero_crossings,
    classify_rotor_state,
    parse_capture,
    plot_zc_snapshot,
    split_complete_dumps,
)

import matplotlib.pyplot as plt


def capture_metrics(cap) -> dict:
    rotor = classify_rotor_state(cap)
    try:
        sectors, _s, _n = analyze_zero_crossings(cap)
        zc = sum(1 for s in sectors if s.status == "zc")
    except Exception:
        zc = 0
    return {
        "late_swing": rotor.get("late_swing"),
        "bemf_amp": rotor.get("bemf_amp"),
        "plateau_spread": rotor.get("plateau_spread"),
        "zc": zc,
    }


def collect(sweep_dir: Path):
    """Return per-(hz,amp) aggregated metrics. amp from the dump's debug line."""
    groups: dict[tuple[int, float], list[dict]] = defaultdict(list)
    for f in sorted(sweep_dir.glob("f*hz.log")):
        text = f.read_text(encoding="ascii", errors="replace")
        segments, _ = split_complete_dumps(text + "end")  # ensure trailing terminator
        for seg in segments:
            try:
                cap = parse_capture(seg)
            except Exception:
                continue
            hz = cap.debug.get("hz")
            amp = cap.debug.get("amp")
            if not hz or not amp:
                continue
            try:
                key = (int(hz), int(amp) / 10.0)
            except ValueError:
                continue
            groups[key].append(capture_metrics(cap))
    return groups


def grid(groups, reducer, key):
    hzs = sorted({k[0] for k in groups})
    amps = sorted({k[1] for k in groups})
    z = np.full((len(amps), len(hzs)), np.nan)
    hz_idx = {h: i for i, h in enumerate(hzs)}
    amp_idx = {a: i for i, a in enumerate(amps)}
    for (hz, amp), recs in groups.items():
        vals = [r[key] for r in recs if r.get(key) is not None]
        if vals:
            z[amp_idx[amp], hz_idx[hz]] = reducer(vals)
    return np.array(hzs), np.array(amps), z


def heatmap(ax, hzs, amps, z, title, cmap="viridis"):
    im = ax.imshow(z, origin="lower", aspect="auto", cmap=cmap,
                   extent=[hzs.min() - 10, hzs.max() + 10, amps.min(), amps.max()])
    ax.set_title(title, fontsize=9)
    ax.set_xlabel("hz")
    ax.set_ylabel("amp %")
    plt.colorbar(im, ax=ax, fraction=0.046)


def main() -> int:
    ap = argparse.ArgumentParser(description="Analyze a scope_sweep run")
    ap.add_argument("sweep_dir", type=Path)
    ap.add_argument("--render", action="store_true", help="render ZC images for flagged points")
    ap.add_argument("--top", type=int, default=30, help="how many interesting points to flag")
    ap.add_argument("--zc-window", type=int, default=3)
    args = ap.parse_args()

    groups = collect(args.sweep_dir)
    if not groups:
        print("no captures parsed")
        return 1
    print(f"parsed {sum(len(v) for v in groups.values())} captures over "
          f"{len({k[0] for k in groups})} freqs x {len({k[1] for k in groups})} amps")

    # Maps: mean late_swing, mean bemf_amp, mean ZC count, mean sensing spread,
    # and snapshot spread of late_swing (bistability / marginal lock).
    fig, axes = plt.subplots(2, 3, figsize=(18, 9))
    for ax, (key, red, ttl, cmap) in zip(axes.flat, [
        ("late_swing", np.mean, "late_swing (mean)", "viridis"),
        ("bemf_amp", np.mean, "bemf_amp (mean)", "viridis"),
        ("zc", np.mean, "in-window ZC count (mean)", "plasma"),
        ("plateau_spread", np.mean, "sensing spread % (low=good)", "viridis_r"),
        ("late_swing", np.std, "late_swing SPREAD across snaps (bistability)", "inferno"),
        ("bemf_amp", np.std, "bemf_amp spread across snaps", "inferno"),
    ]):
        hzs, amps, z = grid(groups, red, key)
        heatmap(ax, hzs, amps, z, ttl, cmap)
    fig.suptitle(f"sweep map: {args.sweep_dir.name}")
    fig.tight_layout()
    out = args.sweep_dir / "sweep_map.png"
    fig.savefig(out, dpi=110)
    plt.close(fig)
    print(f"wrote {out}")

    # Interest score: bistability (snap spread of late_swing) + amp-gradient of
    # late_swing (sharp transition). Normalize each, sum, rank.
    keys = sorted(groups)
    by_hz: dict[int, list] = defaultdict(list)
    for (hz, amp) in keys:
        by_hz[hz].append(amp)
    scored = []
    for (hz, amp), recs in groups.items():
        ls = [r["late_swing"] for r in recs if r.get("late_swing") is not None]
        spread = float(np.std(ls)) if len(ls) > 1 else 0.0
        # amp-gradient: difference to the next amp step at this hz
        amps_here = sorted(by_hz[hz])
        grad = 0.0
        if amp in amps_here:
            i = amps_here.index(amp)
            nb = [a for a in (amps_here[i - 1] if i > 0 else None, amps_here[i + 1] if i + 1 < len(amps_here) else None) if a is not None]
            mean_here = float(np.mean(ls)) if ls else 0.0
            for a2 in nb:
                ls2 = [r["late_swing"] for r in groups[(hz, a2)] if r.get("late_swing") is not None]
                if ls2:
                    grad = max(grad, abs(mean_here - float(np.mean(ls2))))
        scored.append(((hz, amp), spread, grad))

    sp_max = max((s for _, s, _ in scored), default=1.0) or 1.0
    gr_max = max((g for _, _, g in scored), default=1.0) or 1.0
    scored.sort(key=lambda r: r[1] / sp_max + r[2] / gr_max, reverse=True)
    flagged = scored[: args.top]

    flag_path = args.sweep_dir / "interesting.csv"
    with flag_path.open("w", encoding="ascii") as fh:
        fh.write("hz,amp_pct,snap_spread,amp_gradient,score\n")
        for (hz, amp), spread, grad in flagged:
            fh.write(f"{hz},{amp:.1f},{spread:.1f},{grad:.1f},{spread/sp_max + grad/gr_max:.3f}\n")
    print(f"wrote {flag_path} ({len(flagged)} interesting points)")
    print("top points (hz, amp%): " + ", ".join(f"{hz}/{amp:.1f}" for (hz, amp), _, _ in flagged[:10]))

    if args.render:
        img_dir = args.sweep_dir / "flagged_img"
        img_dir.mkdir(exist_ok=True)
        # render the first parseable snapshot at each flagged point
        rendered = 0
        for (hz, amp), _, _ in flagged:
            for f in sorted(args.sweep_dir.glob("f*hz.log")):
                segs, _ = split_complete_dumps(f.read_text(encoding="ascii", errors="replace") + "end")
                hit = None
                for seg in segs:
                    try:
                        cap = parse_capture(seg)
                    except Exception:
                        continue
                    if cap.debug.get("hz") == str(hz) and cap.debug.get("amp") == str(int(round(amp * 10))):
                        hit = cap
                        break
                if hit is not None:
                    hit.debug.setdefault("mode", "six-step")
                    try:
                        plot_zc_snapshot(hit, img_dir / f"f{hz}_a{amp:.1f}.png", smooth_window=args.zc_window)
                        rendered += 1
                    except Exception as exc:
                        print(f"render {hz}/{amp} failed: {exc}")
                    break
        print(f"rendered {rendered} flagged images -> {img_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
