"""AM32-vs-minz failure autopsy figure (2026-07-18).

Four panels from the same-bench same-hour captures:
  1. The climb: interval vs cumulative motor time - AM32 completes,
     minz dies (death markers).
  2. Tracking error (rolling p95 of |measured-estimate|/estimate).
  3. THE PARITY SIGNATURE: rolling (even-sector period - odd-sector
     period) - the +55 us alternation that is ours alone.
  4. The discriminator: qualification-miss rate per polarity.

Usage: python scripts/plot_am32_diff.py
Writes captures/am32_vs_minz_autopsy.png
"""

import csv
import statistics as st

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt


def load_minz(path):
    rows = [r for r in csv.DictReader(open(path))
            if 0 < float(r["duty"]) <= 3332 and float(r["est_before_us"]) > 0]
    t = 0.0
    out = []
    for r in rows:
        per = float(r["period_us"])
        t += per / 1e6
        out.append((t, per, float(r["est_before_us"]), int(r["sector"])))
    return out


def load_am32(path):
    rows = [r for r in csv.DictReader(open(path))
            if float(r["avg_us"]) > 0]
    t = 0.0
    out = []
    for r in rows:
        ci = float(r["ci_us"])
        if not (20 <= ci <= 30000):
            continue
        t += ci / 1e6
        out.append((t, ci, float(r["avg_us"]), int(r["step"])))
    return out


def rolling(vals, n):
    out = []
    for i in range(len(vals)):
        lo = max(0, i - n)
        out.append(st.mean(vals[lo:i + 1]))
    return out


def rolling_p95(vals, n):
    out = []
    for i in range(0, len(vals), n):
        w = sorted(vals[i:i + n])
        if w:
            out.append(w[int(len(w) * 0.95)])
    return out


mz = load_minz("captures/mzt_final2_trace.csv")
am = load_am32("captures/zct_today_trace.csv")

fig, axes = plt.subplots(4, 1, figsize=(13, 15))
fig.suptitle("minz vs AM32 - same bench, same hour, same 60->80 climb (2026-07-18)",
             fontsize=13)

# Panel 1: the climb
ax = axes[0]
ax.plot([a[0] for a in am], [a[1] for a in am], ",", color="tab:green",
        alpha=0.25, label="AM32 ci (COMPLETES, 459k recs)")
ax.plot([m[0] for m in mz], [m[1] for m in mz], ",", color="tab:red",
        alpha=0.25, label="minz period (best run, dies at end)")
ax.axvline(mz[-1][0], color="tab:red", ls="--", lw=1.5)
ax.annotate("minz death\n(crest class)", (mz[-1][0], 400), color="tab:red")
ax.set_ylim(50, 800)
ax.set_ylabel("interval (us)")
ax.set_xlabel("cumulative motor time (s)")
ax.legend(loc="upper right")
ax.set_title("1. The climb: AM32 completes; minz dies near the top")

# Panel 2: tracking error p95 per 1000-record chunk
ax = axes[1]
am_err = [abs(a[1] - a[2]) / a[2] for a in am]
mz_err = [abs(m[1] - m[2]) / m[2] for m in mz]
am_t = [am[min(i + 500, len(am) - 1)][0] for i in range(0, len(am), 1000)]
mz_t = [mz[min(i + 500, len(mz) - 1)][0] for i in range(0, len(mz), 1000)]
ax.plot(am_t, [100 * v for v in rolling_p95(am_err, 1000)], color="tab:green",
        label="AM32 p95 |ci-avg|/avg")
ax.plot(mz_t, [100 * v for v in rolling_p95(mz_err, 1000)], color="tab:red",
        label="minz p95 |per-est|/est")
ax.set_ylabel("tracking error p95 (%)")
ax.set_xlabel("cumulative motor time (s)")
ax.legend()
ax.set_title("2. Tracking error: the 3-8x quality gap")

# Panel 3: parity signature (rolling even-odd period split)
ax = axes[2]


def parity_split(data, n=600):
    ts, ds = [], []
    for i in range(0, len(data) - n, n):
        w = data[i:i + n]
        ev = [x[1] for x in w if x[3] % 2 == 0]
        od = [x[1] for x in w if x[3] % 2 == 1]
        if len(ev) > 20 and len(od) > 20:
            ts.append(w[n // 2][0])
            ds.append(st.mean(ev) - st.mean(od))
    return ts, ds


t_a, d_a = parity_split(am)
t_m, d_m = parity_split(mz)
ax.plot(t_a, d_a, color="tab:green", label="AM32 (zero)")
ax.plot(t_m, d_m, color="tab:red", label="minz (the +55us bias, comp-27 applied)")
ax.axhline(0, color="gray", lw=0.5)
ax.set_ylabel("even - odd period (us)")
ax.set_xlabel("cumulative motor time (s)")
ax.legend()
ax.set_title("3. THE PARITY SIGNATURE: polarity-split periods - ours alone")

# Panel 4: discriminator miss rates
ax = axes[3]
bars = ax.bar(["minz even\n(open loop)", "minz odd\n(open loop)",
               "AM32 even", "AM32 odd"],
              [87, 52, 1, 1],
              color=["tab:red", "salmon", "tab:green", "lightgreen"])
ax.bar_label(bars, fmt="%d%%")
ax.set_ylabel("windows with NO qualified ZC (%)")
ax.set_title("4. The discriminator: qualification-miss rate per polarity\n"
             "(edges PRESENT at the same window fraction in both - "
             "the persistence LEVEL check rejects them 4x asymmetrically)")

plt.tight_layout()
out = "captures/am32_vs_minz_autopsy.png"
plt.savefig(out, dpi=110)
print("wrote", out)
