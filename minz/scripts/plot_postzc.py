"""post-ZC comparator fraction vs throttle: clone (healthy locker,
grabbed 70/95/100%) vs rm32 battery-wall lock-loss capture. The
bedrock: clone plateaus ~40% (ZC lands late in the window, healthy
advance); rm32 = 83% (ZC already passed at window open = LATE
commutation). Same physical quantity (raw != rising), normalized
in-firmware on both sides."""

import pathlib
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

CAP = pathlib.Path(__file__).resolve().parents[1] / "captures"

clone = [(70, 26.8), (95, 38.8), (100, 40.0)]
rm32 = [(100, 83.0)]  # battery-wall lock-loss capture (~80-100%)

fig, ax = plt.subplots(figsize=(9, 5.4))
ax.axhspan(0, 50, color="#1a7f4b", alpha=0.08)
ax.axhspan(70, 100, color="#c0392b", alpha=0.08)
ax.axhline(50, color="#1a7f4b", ls=":", lw=1)
ax.axhline(70, color="#c0392b", ls=":", lw=1)
ax.text(71, 47, "healthy", color="#1a7f4b", fontsize=9)
ax.text(71, 73, "LATE commutation", color="#c0392b", fontsize=9)

cx = [t for t, _ in clone]
cy = [v for _, v in clone]
ax.plot(cx, cy, "o-", color="#2a78d6", lw=2, ms=7, label="clone (holds lock)")
for t, v in clone:
    ax.annotate(f"{v:.0f}%", (t, v), textcoords="offset points", xytext=(0, 8),
                ha="center", fontsize=9, color="#2a78d6")
ax.plot([t for t, _ in rm32], [v for _, v in rm32], "D", color="#e34948", ms=11,
        label="rm32 (wall / OldRoutine lock-loss)")
ax.annotate("83%", (100, 83), textcoords="offset points", xytext=(12, 0),
            va="center", fontsize=10, color="#e34948", fontweight="bold")

ax.set_xlabel("throttle (%)")
ax.set_ylabel("comparator post-ZC fraction (%)")
ax.set_ylim(0, 100)
ax.set_xlim(60, 108)
ax.set_title("rm32 top-end wall = LATE COMMUTATION, not analog silence or CPU budget\n"
             "post-ZC fraction, matched quantity, clone = known-good baseline", fontsize=11)
ax.legend(loc="center left", fontsize=9)
ax.grid(alpha=0.3)
fig.tight_layout()
out = CAP / "postzc_verdict.png"
fig.savefig(out, dpi=110)
print(f"wrote {out}")
