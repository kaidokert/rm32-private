"""Battery full-envelope plot: merge the batt map CSVs (top-rung file
wins where rungs overlap — it's the post-OC-raise unclipped run),
overlay the 11.8 V Extech re-qual and the 8.2 V July baseline.
3-panel PNG: f_e / current / vbat vs throttle."""

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


def merge(names):
    out = {}
    for n in names:
        try:
            out.update(load(n))
        except FileNotFoundError:
            pass
    return out


batt = merge(["map_batt_map40.csv", "map_batt_full.csv", "map_batt_top.csv"])
v118 = merge(["map_v118_map.csv", "map_v118_map8.csv", "map_v118_map85.csv", "map_v118_map90.csv"])
base = load("map_cmp_clone.csv")

SERIES = [
    (base, "#b9c0c9", "clone @ 8.2 V Extech (July)"),
    (v118, "#7fa8d9", "clone @ 11.8 V Extech (re-qual)"),
    (batt, "#1a7f4b", "clone @ 3S battery (unclipped)"),
]
fig, axes = plt.subplots(3, 1, figsize=(10.5, 9.5), sharex=True)
for ax, (label, i) in zip(axes, [("f_e (Hz)", 0), ("current (mA)", 1), ("vbat (mV)", 2)]):
    for data, color, name in SERIES:
        rungs = sorted(data)
        ax.plot(rungs, [data[r][i] for r in rungs], "o-", color=color, label=name, ms=4)
    ax.set_ylabel(label)
    ax.grid(alpha=0.3)
axes[0].annotate("3145 Hz @ 100%\n53 µs commutation\n(all-time record)",
                 xy=(100, 3145), xytext=(70, 2500), fontsize=9, color="#1a7f4b",
                 arrowprops=dict(arrowstyle="->", color="#1a7f4b"))
axes[1].axhline(5100, color="#c0392b", ls="--", lw=1)
axes[1].annotate("Extech 5.1 A ceiling (both supply curves stop under it)",
                 xy=(12, 5300), fontsize=8, color="#c0392b")
axes[1].annotate("7.0 A @ 100% — pack barely working",
                 xy=(100, 7011), xytext=(66, 6300), fontsize=8, color="#1a7f4b",
                 arrowprops=dict(arrowstyle="->", color="#1a7f4b"))
axes[0].legend(loc="upper left", fontsize=9)
axes[2].set_xlabel("throttle (%)")
fig.suptitle("am32_clone battery envelope — three sources, one firmware — 2026-07-26", fontsize=12)
fig.tight_layout()
out = CAP / "batt_envelope.png"
fig.savefig(out, dpi=110)
print(f"wrote {out}")
