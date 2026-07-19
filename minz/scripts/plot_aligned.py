"""Field-aligned AM32-vs-minz trace comparison (operator doctrine:
the two traces log the SAME quantities; the plots must line up).

Pairs (identical definitions):
  minz raw_iv  <->  AM32 zt   (raw ZC-to-ZC interval)
  minz est     <->  AM32 ci   (pair-avg IIR estimate, same formula)
  minz stiff   <->  AM32 avg  (6-slot stiff average)
  minz delay   <->  AM32 wait (scheduled commutation delay)

Distributions compared in the shared 90-150 us cruise band.
Writes captures/aligned_quantities.png
"""

import csv
import statistics as st

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

mz = [r for r in csv.DictReader(open("captures/mzt_v3_trace.csv"))
      if 0 < float(r["duty"]) <= 3332 and float(r["stiff_us"]) > 0]
am = [r for r in csv.DictReader(open("captures/zct_today_trace.csv"))
      if float(r["avg_us"]) > 0]

mzb = [r for r in mz if 90 <= float(r["stiff_us"]) <= 150]
amb = [r for r in am if 90 <= float(r["avg_us"]) <= 150]

pairs = [
    ("raw ZC interval", "raw_iv_us", "zt_us", (60, 260)),
    ("IIR estimate",    "est_us",    "ci_us", (80, 180)),
    ("stiff average",   "stiff_us",  "avg_us", (80, 180)),
    ("scheduled delay", "delay_us",  "wait_us", (0, 80)),
]

fig, axes = plt.subplots(2, 2, figsize=(13, 9))
fig.suptitle("Aligned quantities, shared 90-150us cruise band - minz (red) vs AM32 (green)\n"
             "identical definitions; divergence is real, not methodological", fontsize=12)
for ax, (title, mk, ak, rng) in zip(axes.flat, pairs):
    mv = [float(r[mk]) for r in mzb if 0 < float(r[mk]) < 30000]
    av = [float(r[ak]) for r in amb if 0 < float(r[ak]) < 30000]
    bins = [rng[0] + i * (rng[1] - rng[0]) / 80 for i in range(81)]
    ax.hist(av, bins=bins, density=True, alpha=0.55, color="tab:green",
            label=f"AM32 {ak} (med {st.median(av):.0f}, p95 spread "
                  f"{sorted(av)[int(len(av)*.95)]-sorted(av)[int(len(av)*.05)]:.0f}us)")
    ax.hist(mv, bins=bins, density=True, alpha=0.55, color="tab:red",
            label=f"minz {mk} (med {st.median(mv):.0f}, p95 spread "
                  f"{sorted(mv)[int(len(mv)*.95)]-sorted(mv)[int(len(mv)*.05)]:.0f}us)")
    ax.set_title(title)
    ax.set_xlabel("us")
    ax.legend(fontsize=8)
plt.tight_layout()
plt.savefig("captures/aligned_quantities.png", dpi=110)
print("wrote captures/aligned_quantities.png")

# numeric alignment table
print("\nquantity        minz med/p05/p95        AM32 med/p05/p95")
for title, mk, ak, _ in pairs:
    mv = sorted(float(r[mk]) for r in mzb if 0 < float(r[mk]) < 30000)
    av = sorted(float(r[ak]) for r in amb if 0 < float(r[ak]) < 30000)
    def q(v, f): return v[int(len(v) * f)]
    print(f"{title:15s} {st.median(mv):6.0f}/{q(mv,.05):5.0f}/{q(mv,.95):5.0f}      "
          f"{st.median(av):6.0f}/{q(av,.05):5.0f}/{q(av,.95):5.0f}")
