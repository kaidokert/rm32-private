"""11.8 V bench re-qual plot: merge the v118 map CSVs (best value per
rung), overlay the 8.2 V clone baseline (map_cmp_clone.csv), render a
3-panel PNG (f_e / current / vbat vs throttle)."""

import csv
import pathlib

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

CAP = pathlib.Path(__file__).resolve().parents[1] / "captures"


def load(name):
    out = {}
    with open(CAP / name, newline="") as fh:
        for r in csv.DictReader(fh):
            out[int(r["rung"])] = (
                float(r["f_e_hz"]),
                float(r["ma"]),
                float(r["mv"]),
            )
    return out


v118 = {}
for f in ("map_v118_map.csv", "map_v118_map8.csv", "map_v118_map85.csv", "map_v118_map90.csv"):
    try:
        v118.update(load(f))
    except FileNotFoundError:
        pass
base = load("map_cmp_clone.csv")

rungs = sorted(v118)
brungs = sorted(base)
fig, axes = plt.subplots(3, 1, figsize=(10.5, 9), sharex=True)
panels = [("f_e (Hz)", 0), ("current (mA)", 1), ("vbat (mV)", 2)]
for ax, (label, i) in zip(axes, panels):
    ax.plot(brungs, [base[r][i] for r in brungs], "o-", color="#8a94a3",
            label="clone @ 8.2 V (July study)")
    ax.plot(rungs, [v118[r][i] for r in rungs], "o-", color="#2a78d6",
            label="clone @ 11.8 V (re-qual)")
    ax.set_ylabel(label)
    ax.grid(alpha=0.3)
axes[0].axhline(2315, color="#8a94a3", ls=":", lw=1)
axes[0].annotate("old 100% record (2315 Hz)", xy=(12, 2350), fontsize=8, color="#8a94a3")
axes[0].annotate("2825 Hz @ 85%\n(new record)", xy=(85, 2825), xytext=(60, 2500),
                 fontsize=9, color="#2a78d6",
                 arrowprops=dict(arrowstyle="->", color="#2a78d6"))
axes[1].axhline(5100, color="#c0392b", ls="--", lw=1)
axes[1].annotate("Extech 5.1 A ceiling — 90% folds here (guard kill, as designed)",
                 xy=(12, 5200), fontsize=8, color="#c0392b")
axes[0].legend(loc="upper left", fontsize=9)
axes[2].set_xlabel("throttle (%)")
fig.suptitle("am32_clone 11.8 V bench re-qual vs 8.2 V baseline — 2026-07-26", fontsize=12)
fig.tight_layout()
out = CAP / "v118_requal.png"
fig.savefig(out, dpi=110)
print(f"wrote {out}")
