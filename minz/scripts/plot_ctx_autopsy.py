"""Render a CTX-ring autopsy (the 4-channel per-PWM-cycle cdump) as a
3-panel PNG: full-window current overview, onset zoom with sector
holds marked, and the per-cycle current-step trace.

Usage:
  python scripts/plot_ctx_autopsy.py captures/tautopsy5_session.log \
      --tag tautopsy5 [--zoom 1190 1300]
"""

import argparse
import pathlib
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import magpie  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("infile")
    ap.add_argument("--tag", default="ctx_autopsy")
    ap.add_argument("--zoom", nargs=2, type=int, default=None,
                    help="frame range for the zoom panel")
    a = ap.parse_args()

    text = pathlib.Path(a.infile).read_bytes().decode("utf-8", "replace")
    frames, hz = magpie.parse_cdump(text)
    n = len(frames)
    t_ms = [k * 1000.0 / hz for k in range(n)]
    i_ma = [magpie.raw_to_ma(f["i"]) for f in frames]
    sec = [f["sector"] for f in frames]
    comp = [f["comp"] for f in frames]

    # Auto-onset: first frame beyond 2x the first-10ms baseline.
    base_n = int(hz / 100)
    base = sum(f["i"] for f in frames[:base_n]) / base_n
    thr = max(2 * base, base + 30)
    onset = next((k for k, f in enumerate(frames) if f["i"] > thr), None)

    if a.zoom:
        z0, z1 = a.zoom
    elif onset:
        z0, z1 = max(0, onset - 30), min(n, onset + 80)
    else:
        z0, z1 = 0, min(n, 200)

    fig, axes = plt.subplots(3, 1, figsize=(13, 10))
    fig.suptitle(
        f"{a.tag} — CTX-ring autopsy ({n} PWM cycles @ {hz/1000:.0f} kHz, "
        f"{n*1000/hz:.0f} ms)",
        fontsize=13,
    )

    ax = axes[0]
    ax.plot(t_ms, [v / 1000 for v in i_ma], lw=0.6, color="crimson")
    ax.set_ylabel("current (A)")
    ax.set_xlabel("time (ms)")
    ax.set_title("full pre-trigger window — mid-ON shunt current per PWM cycle")
    if onset:
        ax.axvline(t_ms[onset], color="k", ls="--", lw=0.8)
        ax.annotate(
            f"onset {t_ms[onset]:.1f} ms",
            (t_ms[onset], max(i_ma) / 1000 * 0.85),
            fontsize=9,
        )
    ax.grid(alpha=0.3)

    ax = axes[1]
    zt = t_ms[z0:z1]
    ax.step(zt, [v / 1000 for v in i_ma[z0:z1]], where="post",
            color="crimson", lw=1.2, label="current (A)")
    ax.set_ylabel("current (A)")
    ax2 = ax.twinx()
    ax2.step(zt, sec[z0:z1], where="post", color="steelblue", lw=0.9,
             alpha=0.8, label="sector")
    ax2.step(zt, [c * 0.5 + 6.5 for c in comp[z0:z1]], where="post",
             color="gray", lw=0.7, alpha=0.7, label="comp (offset)")
    ax2.set_ylabel("sector / comp", color="steelblue")
    ax2.set_ylim(-0.5, 8)
    # mark sector holds >= 4 frames
    run_start = z0
    for k in range(z0 + 1, z1 + 1):
        if k == z1 or sec[k] != sec[run_start]:
            if k - run_start >= 4:
                ax.axvspan(t_ms[run_start], t_ms[min(k, n - 1)],
                           color="orange", alpha=0.25)
            run_start = k
    ax.set_title(
        "onset zoom — orange spans = sector HELD >=4 cycles (the rotor-"
        "clocked WAIT); current ramps while the drive is parked"
    )
    ax.set_xlabel("time (ms)")
    ax.grid(alpha=0.3)

    ax = axes[2]
    jumps = [(i_ma[k + 1] - i_ma[k]) / 1000 for k in range(z0, min(z1, n - 1))]
    ax.bar(zt[: len(jumps)], jumps, width=0.03, color="darkorange")
    ax.set_ylabel("dI per cycle (A)")
    ax.set_xlabel("time (ms)")
    ax.set_title("per-PWM-cycle current step — the wait pumps amps per cycle")
    ax.grid(alpha=0.3)

    fig.tight_layout()
    out = pathlib.Path("captures") / f"{a.tag}_autopsy.png"
    fig.savefig(out, dpi=130)
    print(f"wrote {out}")
    if onset:
        print(f"onset frame {onset} ({t_ms[onset]:.1f} ms), baseline "
              f"{magpie.raw_to_ma(int(base))/1000:.2f} A, "
              f"peak {max(i_ma)/1000:.2f} A")


if __name__ == "__main__":
    main()
