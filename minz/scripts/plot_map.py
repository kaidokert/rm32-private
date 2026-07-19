"""Overlaid operating-range MAP: minz ladder vs AM32 sweep.

Panels: throttle vs electrical Hz, vs current, vs vbat - both
firmwares on shared axes. minz throttle = amp% (duty of 3332);
AM32 throttle = % (duty of ~2000 scale). Same physical duty axis.

Usage: python scripts/plot_map.py [--minz captures/ladder_minz_s1c.csv]
                                  [--am32 captures/am32_sweep_50_100.csv]
Writes captures/operating_map.png
"""

import argparse
import csv

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

VBAT_UV_PER_COUNT = 7507  # raw -> uV (minz divider cal)
ISNS_MA_PER_COUNT = 26    # approximate raw -> mA (INA1801 x 1.5mohm)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--minz", default="captures/ladder_minz_s1c.csv")
    ap.add_argument("--am32", default="captures/am32_sweep_50_100.csv")
    ap.add_argument("--out", default="captures/operating_map.png")
    a = ap.parse_args()

    mz = [r for r in csv.DictReader(open(a.minz))
          if float(r["f_e_hz"]) > 200]
    am = list(csv.DictReader(open(a.am32)))

    mz_thr = [float(r["amp"]) for r in mz]
    mz_hz = [float(r["f_e_hz"]) for r in mz]
    mz_a = [float(r["isns_raw"]) * ISNS_MA_PER_COUNT / 1000 for r in mz]
    mz_v = [float(r["vbat_raw"]) * VBAT_UV_PER_COUNT / 1e6 for r in mz]

    am_thr = [float(r["pct"]) for r in am]
    am_hz = [float(r["f_e_hz"]) for r in am]
    am_a = [float(r["amps"]) for r in am]
    am_v = [float(r["vbat"]) for r in am]

    fig, axes = plt.subplots(3, 1, figsize=(11, 12), sharex=True)
    fig.suptitle("Operating map - minz (red) vs AM32 (green), same bench/motor/supply")

    axes[0].plot(am_thr, am_hz, "o-", color="tab:green", label="AM32")
    axes[0].plot(mz_thr, mz_hz, ".-", color="tab:red", label="minz")
    axes[0].set_ylabel("electrical Hz")
    axes[0].legend()
    axes[0].set_title("speed vs throttle")

    axes[1].plot(am_thr, am_a, "o-", color="tab:green", label="AM32 (KISS amps)")
    axes[1].plot(mz_thr, mz_a, ".-", color="tab:red", label="minz (isns est)")
    axes[1].set_ylabel("current (A)")
    axes[1].legend()
    axes[1].set_title("current vs throttle")

    axes[2].plot(am_thr, am_v, "o-", color="tab:green", label="AM32")
    axes[2].plot(mz_thr, mz_v, ".-", color="tab:red", label="minz")
    axes[2].set_ylabel("bus V")
    axes[2].set_xlabel("throttle (%)")
    axes[2].legend()
    axes[2].set_title("supply sag vs throttle")

    plt.tight_layout()
    plt.savefig(a.out, dpi=110)
    print("wrote", a.out)


if __name__ == "__main__":
    main()
