"""Battery re-qual charts: f_e vs time for the 10% ladder, 2% ladder,
and double-slam captures (battB_*). Time from unwrapped tenkhz
(19.6 kHz tick); f_e from per-commutation zt; light rolling median."""

import csv
import pathlib
import statistics

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

CAP = pathlib.Path(__file__).resolve().parents[1] / "captures"
TICK = 1.0 / 19607.8  # TIM6 update period (PSC=79, ARR=50 @ 80 MHz)


def load(tag):
    path = CAP / f"{tag}_trace.csv"
    with open(path, newline="") as fh:
        rdr = csv.DictReader(fh)
        cols = rdr.fieldnames
        ztk = next(c for c in cols if "zt" in c.lower())
        thk = next(c for c in cols if "tenkhz" in c.lower())
        dtk = next(c for c in cols if c.lower() == "duty")
        t, fe, duty = [], [], []
        last_raw, unwrapped = None, 0
        for r in rdr:
            try:
                z = float(r[ztk])
                th = int(r[thk])
                d = int(r[dtk])
            except (ValueError, KeyError):
                continue
            if z <= 0:
                continue
            if last_raw is not None:
                delta = (th - last_raw) & 0xFFFF
                if delta > 40000:  # decode glitch guard
                    delta = 0
                unwrapped += delta
            last_raw = th
            t.append(unwrapped * TICK)
            fe.append(1e6 / (6.0 * (z / 2.0)))  # zt in 0.5 µs ticks
            duty.append(d)
    return t, fe, duty


def med_smooth(v, w=15):
    h = w // 2
    return [statistics.median(v[max(0, i - h):i + h + 1]) for i in range(len(v))]


RUNS = [
    ("battB_lad10", "10% ladder 10→100→10", "#1a7f4b"),
    ("battB_lad2", "2% ladder 10→100→10", "#2a78d6"),
    ("battB_slam15", "double full slam 10⇎100", "#c0392b"),
]

for tag, title, color in RUNS:
    t, fe, duty = load(tag)
    fes = med_smooth(fe)
    fig, ax1 = plt.subplots(figsize=(11, 5.2))
    ax1.plot(t, fes, color=color, lw=1.2, label="f_e (15-rec median)")
    ax1.set_xlabel("time (s)")
    ax1.set_ylabel("electrical frequency (Hz)", color=color)
    ax1.grid(alpha=0.3)
    ax2 = ax1.twinx()
    ax2.plot(t, duty, color="#8a94a3", lw=0.8, alpha=0.7, label="duty (0..2000)")
    ax2.set_ylabel("duty", color="#8a94a3")
    ax1.set_title(f"{tag} — {title} — 3S pack, {len(fe):,} commutations")
    fig.tight_layout()
    out = CAP / f"{tag}.png"
    fig.savefig(out, dpi=110)
    plt.close(fig)
    print(f"wrote {out} ({len(fe)} recs, span {t[-1]:.1f}s)")
