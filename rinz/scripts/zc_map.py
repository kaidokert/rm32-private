#!/usr/bin/env python3
"""(freq, amp) map of the BEMF zero-crossing / load angle from the validated
3-harmonic fit -- the "observation" sweep map (sweep_map.py maps *lock*; this maps
where the rotor sits relative to commutation, i.e. the thing the whole open-loop
effort is about).

For each capture it fits the float-window BEMF to N harmonics (default 3, the count
zc_validate showed matches the direct in-window ZC with zero outliers), reads off
each sector's crossing position (% of the 60-deg window; 50% = window centre), and
reduces to a few per-capture scalars, aggregated over the (freq, amp) plane:

  load_angle    -- mean crossing offset from centre across the 6 sectors (deg elec):
                   how far the rotor lags/leads the commutation. THE load-angle map.
  sector_spread -- spread of the per-sector offsets (the structural asymmetry we've
                   chased; should shrink if the observation is clean).
  residual      -- median |in-window ZC - fit| where ground truth exists: re-validates
                   the reconstruction at EACH (freq, amp), not just the one amp we
                   checked. Should stay small everywhere if the fit is trustworthy.
  n_inwin       -- count of directly-observable in-window crossings (catch boundary).

Also pulls iu_ma / vbus_mv from the debug line. Renders a panel of heatmaps.

    python scripts/zc_map.py logs/sweep_<ts> --render
    python scripts/zc_map.py logs/stream_xxx.log --amp-bin 1   # 1-D slice / stream
"""

from __future__ import annotations

import argparse
import statistics
from collections import defaultdict
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parent))
import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

from scope_common import (
    PHASE_TO_CHANNEL,
    _driven_pair_neutral,
    analyze_zero_crossings,
    lowpass_channels,
)
from zc_validate import SECTOR, TWO_PI, _float_phase, crossing_pct, fit_harmonics, load_captures


def capture_zc(cap, *, harmonics: int = 3, smooth: int = 3, blank: int = 2):
    """Per-capture load-angle scalars from the N-harmonic float-window fit, or None."""
    hz = float(cap.debug.get("hz", "0") or 0)
    if hz <= 0 or len(cap.channels) < 3:
        return None
    sm = lowpass_channels(cap.channels, smooth)
    frames = min(len(c) for c in sm)
    fps = cap.sample_hz / (hz * 6.0)
    fpr = cap.sample_hz / hz
    neutral = _driven_pair_neutral(sm, frames, fps)
    sectors, _, _ = analyze_zero_crossings(cap, smooth_window=smooth, blank_frames=blank)

    samp = defaultdict(lambda: ([], []))
    for i in range(frames):
        k = int(i / fps)
        if (i - k * fps) < blank:
            continue
        fl = _float_phase(k % 6)
        samp[fl][0].append((i / fpr) * TWO_PI)
        samp[fl][1].append(sm[PHASE_TO_CHANNEL[fl]][i] - neutral[i])
    coef = {
        fl: fit_harmonics(a, v, harmonics)
        for fl, (a, v) in samp.items()
        if len(a) >= 2 * harmonics + 4
    }
    zin = defaultdict(list)
    for sec in sectors:
        if sec.status == "zc" and sec.zc_pct is not None:
            zin[sec.index % 6].append(sec.zc_pct)

    offs, resid, n_inwin = [], [], 0
    for s in range(6):
        fl = _float_phase(s)
        if fl not in coef:
            continue
        zc = crossing_pct(coef[fl], harmonics, s)
        if zc is None:
            continue
        offs.append(zc - 50.0)  # % deviation from window centre
        for z in zin.get(s, []):
            n_inwin += 1
            resid.append(abs(z - zc))
    if not offs:
        return None

    def _int(key):
        try:
            return int(cap.debug.get(key))
        except (TypeError, ValueError):
            return None

    return {
        "load_angle": statistics.mean(offs) * 0.6,  # deg elec (1% window = 0.6 deg)
        "sector_spread": (max(offs) - min(offs)) * 0.6,
        "residual": statistics.median(resid) if resid else None,
        "n_inwin": n_inwin,
        "iu_ma": _int("iu_ma"),
        "vbus_mv": _int("vbus_mv"),
    }


def grid(groups, key, reducer=np.nanmean):
    hzs = sorted({k[0] for k in groups})
    amps = sorted({k[1] for k in groups})
    z = np.full((len(amps), len(hzs)), np.nan)
    hi = {h: i for i, h in enumerate(hzs)}
    ai = {a: i for i, a in enumerate(amps)}
    for (hz, amp), recs in groups.items():
        vals = [r[key] for r in recs if r.get(key) is not None]
        if vals:
            z[ai[amp], hi[hz]] = reducer(vals)
    return np.array(hzs), np.array(amps), z


def heatmap(ax, hzs, amps, z, title, cmap="viridis"):
    extent = [hzs.min() - 10, hzs.max() + 10, amps.min() - 0.5, amps.max() + 0.5]
    im = ax.imshow(z, origin="lower", aspect="auto", cmap=cmap, extent=extent)
    ax.set_title(title, fontsize=9)
    ax.set_xlabel("hz")
    ax.set_ylabel("amp %")
    plt.colorbar(im, ax=ax, fraction=0.046)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("--harmonics", type=int, default=3)
    ap.add_argument("--smooth", type=int, default=3)
    ap.add_argument("--amp-bin", type=float, default=1.0, help="round amps to this %% for the grid")
    ap.add_argument("--render", action="store_true")
    args = ap.parse_args()

    caps = load_captures(args.paths)
    if not caps:
        raise SystemExit("no parsable captures")

    groups = defaultdict(list)
    for cap in caps:
        r = capture_zc(cap, harmonics=args.harmonics, smooth=args.smooth)
        if r is None:
            continue
        try:
            hz = int(float(cap.debug.get("hz", 0)))
            amp = round(round(int(cap.debug.get("amp", 0)) / 10.0 / args.amp_bin) * args.amp_bin, 2)
        except (TypeError, ValueError):
            continue
        groups[(hz, amp)].append(r)
    if not groups:
        raise SystemExit("no usable captures (need six-step dumps with hz/amp)")

    n = sum(len(v) for v in groups.values())
    print(f"{n} captures over {len({k[0] for k in groups})} freqs x {len({k[1] for k in groups})} amps")

    # validation re-check across the whole plane (the caveat from the single-amp run)
    allres = [r["residual"] for v in groups.values() for r in v if r.get("residual") is not None]
    if allres:
        allres.sort()
        print(f"validation residual (|in-window ZC - {args.harmonics}h fit|): "
              f"median {allres[len(allres)//2]:.1f}%  max {max(allres):.1f}%  "
              f"({sum(1 for x in allres if x > 20)}/{len(allres)} > 20%)  over the whole plane")
    else:
        print("no in-window crossings anywhere -> cannot re-validate (all deep lock).")

    # quick text view: load angle vs hz/amp
    print("\n(hz, amp%) -> load_angle(deg)  sector_spread(deg)  n_inwin")
    for (hz, amp) in sorted(groups):
        recs = groups[(hz, amp)]
        la = statistics.mean(r["load_angle"] for r in recs)
        sp = statistics.mean(r["sector_spread"] for r in recs)
        ni = sum(r["n_inwin"] for r in recs)
        print(f"  {hz:4d}/{amp:5.1f}  {la:+7.1f}        {sp:6.1f}           {ni}")

    if args.render:
        fig, axes = plt.subplots(2, 2, figsize=(13, 9))
        for ax, (key, ttl, cmap) in zip(axes.flat, [
            ("load_angle", "load angle (deg elec, fit crossing vs window centre)", "coolwarm"),
            ("sector_spread", "per-sector spread (deg) -- structural asymmetry", "magma"),
            ("residual", "validation residual % (|in-win ZC - fit|; blank=deep lock)", "viridis_r"),
            ("n_inwin", "in-window ZC count (catch-boundary ground truth)", "plasma"),
        ]):
            reducer = (lambda xs: np.nansum(xs)) if key == "n_inwin" else np.nanmean
            hzs, amps, z = grid(groups, key, reducer)
            heatmap(ax, hzs, amps, z, ttl, cmap)
        fig.suptitle(f"ZC / load-angle map ({args.harmonics}-harmonic fit): {args.paths[0].name}")
        fig.tight_layout()
        out = (args.paths[0] if args.paths[0].is_dir() else args.paths[0].parent) / "zc_map.png"
        fig.savefig(out, dpi=110)
        print(f"\nwrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
