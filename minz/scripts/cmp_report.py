#!/usr/bin/env python3
"""am32_clone vs real AM32 (ZCTRACE fork) comparison report.

Host-only analysis of the captures/cmp_* + captures/map_cmp_* set
(same motor/bench, captured minutes apart). Produces
captures/cmp_report/metrics.json + fig_*.png.

Trace CSV columns (zctrace_capture.py convention): step,old,zt_us,
ci_us,wait_us,duty,tenkhz,avg_us. Records are per-commutation.

Firmware-specific wire facts discovered in this dataset:
  * tenkhz wraps at 65536 on the clone (raw u16) but at 20001 on
    AM32 (the counter resets at 20000 = a 1 s rollover) — unwrap
    modulus is per-firmware.
  * AM32's wire interleaves KISS/SPK frames with trace records; the
    5B A9 resync occasionally decodes junk records (duty=34217,
    wild tenkhz). Filtered by duty sanity + a tenkhz neighbor-
    consistency test.
  * Batch decimation above ~80%: records arrive in bursts; gaps are
    flagged by tenkhz jumps >20 counts and reset the 6-deep rolling
    history (zct_sweep_report.py convention, reused here).

Excursion metric = zct_sweep_report.py convention exactly: for each
record, ref = mean of the previous 6 periods (running-mode records
only, 20..20000 us); upward excursion pct = p/ref - 1; count >12.5%
and >25% per 1k records; history cleared at batch gaps.
"""

import csv
import json
import pathlib
import re
import statistics

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

BASE = pathlib.Path(__file__).resolve().parent.parent / "captures"
OUT = BASE / "cmp_report"
OUT.mkdir(exist_ok=True)

TICK_S = 51.02e-6  # tenkhz count period (19.6 kHz tick)
MOD = {"clone": 65536, "am32": 20001}  # tenkhz wrap modulus
GAP_COUNTS = 20  # tenkhz jump > this = batch-decimation gap

C_CLONE = "#2a78d6"  # blue  (validated pair, CVD dE 74.6)
C_AM32 = "#e34948"  # red
INK = "#0b0b0b"
MUTED = "#898781"
GRID = "#e1e0d9"

plt.rcParams.update({
    "figure.facecolor": "#fcfcfb", "axes.facecolor": "#fcfcfb",
    "axes.edgecolor": "#c3c2b7", "axes.labelcolor": INK,
    "axes.grid": True, "grid.color": GRID, "grid.linewidth": 0.7,
    "xtick.color": MUTED, "ytick.color": MUTED,
    "font.size": 10, "axes.titlesize": 11, "legend.frameon": False,
    "axes.spines.top": False, "axes.spines.right": False,
})

FIGSIZE = (1200 / 110, 700 / 110)
DPI = 110


# ---------------------------------------------------------------- load

def load_trace(name, fw):
    """Load a trace CSV -> list of dicts with unwrapped time t (s).

    Junk-record rejection (AM32 wire garble): duty > 2010 dropped;
    then any single record whose tenkhz is inconsistent with BOTH
    neighbors while the neighbors agree with each other is dropped.
    """
    mod = MOD[fw]
    rows = []
    with open(BASE / name, newline="") as fh:
        for r in csv.DictReader(fh):
            d = int(r["duty"])
            if d > 2010:
                continue  # junk (decoded KISS/SPK bytes)
            rows.append(dict(
                step=int(r["step"]), old=int(r["old"]),
                zt=float(r["zt_us"]), ci=float(r["ci_us"]),
                duty=d, tk=int(r["tenkhz"])))
    # neighbor-consistency junk drop
    keep = []
    n = len(rows)
    for i, r in enumerate(rows):
        if 0 < i < n - 1:
            din = (r["tk"] - rows[i - 1]["tk"]) % mod
            dout = (rows[i + 1]["tk"] - r["tk"]) % mod
            dskip = (rows[i + 1]["tk"] - rows[i - 1]["tk"]) % mod
            if din > 500 and dout > 500 and dskip <= 500:
                continue  # lone tenkhz outlier = junk record
        keep.append(r)
    # unwrap
    t = 0.0
    prev = None
    for r in keep:
        if prev is not None:
            dc = (r["tk"] - prev) % mod
            t += dc * TICK_S
            r["gap"] = dc > GAP_COUNTS
        else:
            r["gap"] = False
        prev = r["tk"]
        r["t"] = t
    return keep


# ------------------------------------------------------- segmentation

def plateau_segments(rows, min_count=800, tol=0):
    """Split into contiguous segments of plateau duty values.

    Plateau duties = duty values carrying >= min_count records
    (the duty column is exact per rung). Records at non-plateau
    duty (ramp transitions) separate but do not join segments.
    Returns list of dicts {duty, i0, i1 (record index span,
    inclusive), t0, t1, recs}.
    """
    from collections import Counter
    cnt = Counter(r["duty"] for r in rows)
    # merge near-identical duties (AM32 low trace has 106 and 107)
    plateaus = {}
    for d, c in sorted(cnt.items()):
        merged = False
        if tol:
            for p in list(plateaus):
                if abs(d - p) <= tol:
                    plateaus[p] += c
                    merged = True
                    break
        if not merged:
            plateaus[d] = c
    plateaus = {d for d, c in plateaus.items() if c >= min_count}

    def canon(d):
        for p in plateaus:
            if abs(d - p) <= tol:
                return p
        return None

    segs = []
    cur = None
    for i, r in enumerate(rows):
        p = canon(r["duty"])
        if p is None:
            continue
        if cur is not None and cur["duty"] == p:
            cur["i1"] = i
            cur["recs"].append(r)
        else:
            if cur is not None:
                segs.append(cur)
            cur = dict(duty=p, i0=i, i1=i, recs=[r])
    if cur is not None:
        segs.append(cur)
    # merge back-to-back same-duty segments split only by ramp recs
    merged = []
    for s in segs:
        if merged and merged[-1]["duty"] == s["duty"]:
            merged[-1]["i1"] = s["i1"]
            merged[-1]["recs"].extend(s["recs"])
        else:
            merged.append(s)
    for s in merged:
        s["t0"] = s["recs"][0]["t"]
        s["t1"] = s["recs"][-1]["t"]
    return merged


def ladder_legs(segs):
    """Attribute ladder segments to up/down legs around the peak-duty
    segment. Leading start-attempt segments (repeats of the lowest
    duty before the monotonic climb) are excluded."""
    peak_i = max(range(len(segs)), key=lambda i: segs[i]["duty"])
    up = [segs[peak_i]]
    d = segs[peak_i]["duty"]
    for i in range(peak_i - 1, -1, -1):
        if segs[i]["duty"] < d:
            up.append(segs[i])
            d = segs[i]["duty"]
        else:
            break
    up.reverse()
    down = []
    d = segs[peak_i]["duty"]
    for i in range(peak_i + 1, len(segs)):
        if segs[i]["duty"] < d:
            down.append(segs[i])
            d = segs[i]["duty"]
        else:
            break
    return up, down


# --------------------------------------------------------- excursions

def excursion_stats(recs):
    """zct_sweep_report.py convention: running-mode records only,
    period 20..20000 us, 6-deep rolling mean, upward excursions,
    history reset at batch-decimation gaps."""
    hist = []
    vals = []
    e125 = e250 = 0
    worst = 0.0
    for r in recs:
        if r["gap"]:
            hist.clear()
        if r["old"] or not (20 <= r["zt"] <= 20000):
            continue
        p = r["zt"]
        if len(hist) == 6:
            ref = sum(hist) / 6
            if ref > 0 and p > ref:
                exc = p / ref - 1.0
                worst = max(worst, exc)
                if exc > 0.125:
                    e125 += 1
                if exc > 0.25:
                    e250 += 1
        hist.append(p)
        if len(hist) > 6:
            hist.pop(0)
        vals.append(p)
    n = len(vals)
    if n < 50:
        return None
    med = statistics.median(vals)
    return dict(n_records=n, med_period_us=round(med, 1),
                fe_hz=round(1e6 / (6 * med), 1),
                exc12_per_1k=round(1e3 * e125 / n, 2),
                exc25_per_1k=round(1e3 * e250 / n, 2),
                worst_pct=round(100 * worst, 1))


def deviations_pct(recs):
    """Per-record deviation (%) vs the 6-deep rolling mean, with
    the same gap/validity conventions. Returns (steps, devs)."""
    hist = []
    out_step, out_dev = [], []
    for r in recs:
        if r["gap"]:
            hist.clear()
        if r["old"] or not (20 <= r["zt"] <= 20000):
            continue
        p = r["zt"]
        if len(hist) == 6:
            ref = sum(hist) / 6
            if ref > 0:
                out_step.append(r["step"])
                out_dev.append(100.0 * (p / ref - 1.0))
        hist.append(p)
        if len(hist) > 6:
            hist.pop(0)
    return out_step, out_dev


# ------------------------------------------------------ step response

def fe_series(recs, smooth=15):
    """(t, fe_hz) from running-mode records, rolling-median smoothed."""
    ts, fs = [], []
    for r in recs:
        if r["old"] or not (20 <= r["zt"] <= 20000):
            continue
        ts.append(r["t"])
        fs.append(1e6 / (6 * r["zt"]))
    if smooth > 1 and len(fs) > smooth:
        half = smooth // 2
        sm = []
        for i in range(len(fs)):
            lo, hi = max(0, i - half), min(len(fs), i + half + 1)
            sm.append(statistics.median(fs[lo:hi]))
        fs = sm
    return ts, fs


def median_in(ts, fs, t0, t1):
    v = [f for t, f in zip(ts, fs) if t0 <= t <= t1]
    return statistics.median(v) if v else None


def analyze_transition(rows, ts, fs, src_seg, dst_seg, dst_duty):
    """Metrics for one setpoint step. dst_seg may be None (clone
    killed / capture truncated before the destination plateau)."""
    src_duty = src_seg["duty"]
    # first duty movement after the source plateau's last record
    t_move = None
    for r in rows[src_seg["i1"] + 1:]:
        if abs(r["duty"] - src_duty) > 5:
            t_move = r["t"]
            break
    base = median_in(ts, fs, src_seg["t1"] - 0.5, src_seg["t1"])
    out = dict(t_step_s=round(t_move, 3) if t_move else None,
               fe_from_hz=round(base, 1) if base else None)
    if dst_seg is None or base is None or t_move is None:
        out.update(fe_to_hz=None, t_10_90_ms=None,
                   overshoot_pct=None, settle_ms=None,
                   note="destination plateau never reached"
                        " (clone: vbat-floor kill mid-ramp)")
        return out
    final = median_in(ts, fs, dst_seg["t1"] - 1.0, dst_seg["t1"])
    delta = final - base
    out["fe_to_hz"] = round(final, 1)
    thr = base + 0.9 * delta
    t90 = None
    for t, f in zip(ts, fs):
        if t < t_move:
            continue
        if t > dst_seg["t1"]:
            break
        if (delta > 0 and f >= thr) or (delta < 0 and f <= thr):
            t90 = t
            break
    out["t_10_90_ms"] = round((t90 - t_move) * 1e3, 1) if t90 else None
    # overshoot: worst excursion past `final` inside the dst plateau,
    # in the step direction, as % of the step delta
    seg_f = [f for t, f in zip(ts, fs)
             if dst_seg["t0"] <= t <= dst_seg["t1"]]
    if seg_f and delta:
        ext = max(seg_f) if delta > 0 else min(seg_f)
        out["overshoot_pct"] = round(
            max(0.0, (ext - final) / delta) * 100, 1)
    # settle: last time (before dst plateau end) the smoothed f_e sits
    # outside the band. Two bands reported: +/-5% of the step delta
    # (settle_ms, the usual control definition) and the literal
    # +/-5% of the final value (settle_final_band_ms) — at low-rpm
    # plateaus commutation jitter exceeds the latter band, so falls
    # read as "never settled" under it.
    for key, band in (("settle_ms", 0.05 * abs(delta)),
                      ("settle_final_band_ms", 0.05 * abs(final))):
        t_settle = t_move
        for t, f in zip(ts, fs):
            if t < t_move or t > dst_seg["t1"]:
                continue
            if abs(f - final) > band:
                t_settle = t
        out[key] = round((t_settle - t_move) * 1e3, 1)
    return out


# --------------------------------------------------------- kill / bb

def parse_kill(raw_name):
    b = (BASE / raw_name).read_bytes()
    s = b.decode("latin1")
    m = re.search(r"!! KILL reason=(\d+) \(1=OC 2=vbat\) "
                  r"iraw=(\d+) vbat=(\d+)", s)
    if not m:
        return None
    off = m.start()
    events = []
    t_cum = 0
    for bm in re.finditer(
            r"bb \+\s*(\d+)us (REF|ACC|DSY|BLD|DRK|NOZ|DIS|ENG|STV|RAQ)"
            r" s(\d) d=(\d+)", s[off:]):
        dt = int(bm.group(1))
        t_cum += dt
        events.append(dict(dt_us=dt, t_us=t_cum, type=bm.group(2),
                           sector=int(bm.group(3)), d=int(bm.group(4))))
    # minz sense calibration (map_sweep.py)
    ma_per = 3300.0 / 4095.0 / 30.0 * 1000.0
    mv_per = 3300.0 / 4095.0 * 9.33
    return dict(reason=int(m.group(1)),
                reason_name={1: "overcurrent", 2: "vbat"}[int(m.group(1))],
                iraw=int(m.group(2)), vbat_raw=int(m.group(3)),
                i_ma=round(int(m.group(2)) * ma_per),
                vbat_mv=round(int(m.group(3)) * mv_per),
                byte_offset=off, file_bytes=len(b),
                byte_offset_fraction=round(off / len(b), 5),
                bb_events=events)


# =============================================================== main

metrics = {"conventions": {
    "tick_s": TICK_S, "tenkhz_modulus": MOD, "gap_counts": GAP_COUNTS,
    "excursion": "zct_sweep_report.py: upward vs 6-deep rolling mean,"
                 " running-mode records, history reset at batch gaps",
    "settle_band": "+/-5% of final f_e",
    "junk_filter": "duty>2010 dropped + lone tenkhz-outlier records"
                   " dropped (AM32 wire garble decoding as trace recs)",
}}

# ---- map ------------------------------------------------------------
def load_map(name):
    out = {}
    with open(BASE / name, newline="") as fh:
        for r in csv.DictReader(fh):
            out[int(r["rung"])] = dict(fe=float(r["f_e_hz"]),
                                       ma=float(r["ma"]),
                                       mv=float(r["mv"]))
    return out


map_c = load_map("map_cmp_clone.csv")
map_a = load_map("map_cmp_am32.csv")
map_rows = []
for rung in sorted(set(map_c) | set(map_a)):
    c = map_c.get(rung)
    a = map_a.get(rung)
    map_rows.append(dict(
        throttle=rung,
        fe_clone=c["fe"] if c else None, fe_am32=a["fe"] if a else None,
        ma_clone=c["ma"] if c else None, ma_am32=a["ma"] if a else None,
        mv_clone=c["mv"] if c else None, mv_am32=a["mv"] if a else None,
        fe_ratio=round(c["fe"] / a["fe"], 3) if c and a else None))
metrics["map"] = map_rows

# ---- ladder ---------------------------------------------------------
lad = {}
for fw, name in [("clone", "cmp_clone_ladder_trace.csv"),
                 ("am32", "cmp_am32_ladder_trace.csv")]:
    rows = load_trace(name, fw)
    segs = plateau_segments(rows, min_count=2000)
    up, down = ladder_legs(segs)
    duties = sorted({s["duty"] for s in up + down})
    # rank -> throttle % (ladder = 10..100 in 10% rungs)
    pct_of = {d: (i + 1) * 10 for i, d in enumerate(duties)}
    per_rung = []
    for leg, ss in [("up", up), ("down", down)]:
        for s in ss:
            st = excursion_stats(s["recs"])
            if st:
                per_rung.append(dict(throttle=pct_of[s["duty"]],
                                     leg=leg, duty=s["duty"],
                                     dwell_s=round(s["t1"] - s["t0"], 2),
                                     **st))
    allrecs = [r for s in up + down for r in s["recs"]]
    lad[fw] = dict(rows=rows, segs=segs, up=up, down=down,
                   pct_of=pct_of, per_rung=per_rung)
    metrics.setdefault("ladder", {})[fw] = dict(
        per_rung=per_rung,
        aggregate=excursion_stats(allrecs),
        note=("capture ends during the 90% up-leg rung; 100% rung and"
              " the entire down leg are absent from the AM32 trace."
              " The 10% up-leg span includes the pre-ladder start/"
              "settle dwell at the same duty."
              if fw == "am32" else
              "all 19 rungs present; the 10% up-leg span includes"
              " the pre-ladder start/settle dwell at the same duty"))

# sanity: map f_e vs ladder up-leg plateau f_e
sanity = []
for fw in ("clone", "am32"):
    mp = map_c if fw == "clone" else map_a
    for pr in metrics["ladder"][fw]["per_rung"]:
        if pr["leg"] != "up":
            continue
        rung = pr["throttle"]
        if rung in mp:
            d = 100 * (mp[rung]["fe"] - pr["fe_hz"]) / pr["fe_hz"]
            sanity.append(dict(fw=fw, throttle=rung,
                               fe_map=mp[rung]["fe"],
                               fe_ladder=pr["fe_hz"],
                               delta_pct=round(d, 2)))
metrics["map_vs_ladder_delta"] = sanity

# ---- steps ----------------------------------------------------------
step_files = [
    ("clone", "90", "cmp_clone_step90_trace.csv",
     "cmp_clone_step90_raw.bin"),
    ("clone", "100", "cmp_clone_step100b_trace.csv",
     "cmp_clone_step100b_raw.bin"),
    ("clone", "100_earlier", "cmp_clone_step_trace.csv",
     "cmp_clone_step_raw.bin"),
    ("am32", "90", "cmp_am32_step90_trace.csv", None),
    ("am32", "100", "cmp_am32_step100_trace.csv", None),
]
steps = {}
step_data = {}
for fw, prof, name, raw in step_files:
    rows = load_trace(name, fw)
    segs = plateau_segments(rows, min_count=3000)
    ts, fs = fe_series(rows)
    # expected plateau sequence: lo(10%) [start+dwell], 60%, lo, hi
    lo = min(s["duty"] for s in segs)
    mids = [s for s in segs if s["duty"] != lo]
    mid_seg = mids[0] if mids else None  # 60%
    lo_segs = [s for s in segs if s["duty"] == lo]
    hi_segs = [s for s in mids[1:] if s["duty"] > (mid_seg["duty"]
               if mid_seg else 0)]
    hi_seg = hi_segs[0] if hi_segs else None
    tr = {}
    if mid_seg and lo_segs:
        tr["10_to_60"] = analyze_transition(
            rows, ts, fs, lo_segs[0], mid_seg, mid_seg["duty"])
        after = [s for s in lo_segs if s["t0"] > mid_seg["t1"]]
        if after:
            tr["60_to_10"] = analyze_transition(
                rows, ts, fs, mid_seg, after[0], lo)
            hi_lbl = f"10_to_{prof.split('_')[0]}"
            tr[hi_lbl] = analyze_transition(
                rows, ts, fs, after[0], hi_seg,
                hi_seg["duty"] if hi_seg else None)
            if hi_seg:
                back = [s for s in lo_segs if s["t0"] > hi_seg["t1"]]
                tr[f"{prof.split('_')[0]}_to_10"] = (
                    analyze_transition(rows, ts, fs, hi_seg,
                                       back[0] if back else None, lo)
                    if back else
                    dict(note="capture ends during/after the high"
                              " dwell; fall not recorded"))
    kill = parse_kill(raw) if raw else None
    steps[f"{fw}_step{prof}"] = dict(
        transitions=tr,
        trace_end_s=round(rows[-1]["t"], 2),
        killed=bool(kill),
        kill_banner=(dict(iraw=kill["iraw"], vbat_raw=kill["vbat_raw"],
                          i_ma=kill["i_ma"], vbat_mv=kill["vbat_mv"])
                     if kill else None))
    step_data[f"{fw}_{prof}"] = dict(rows=rows, segs=segs, ts=ts,
                                     fs=fs, lo=lo, mid=mid_seg,
                                     hi=hi_seg, lo_segs=lo_segs)
metrics["steps"] = steps
metrics["steps_note"] = (
    "both firmwares run the SAME AM32 throttle-ramp algorithm on"
    " instant setpoint steps; all three clone high-step captures end"
    " in a vbat-floor kill during the 10->90/100 ramp (the clone's"
    " own low-voltage guard), while no AM32 capture shows a kill")

# ---- kill exhibit ---------------------------------------------------
metrics["kill_exhibit"] = parse_kill("cmp_clone_step100b_raw.bin")
metrics["kill_exhibit_others"] = {
    f: (lambda k: dict(iraw=k["iraw"], vbat_raw=k["vbat_raw"],
                       i_ma=k["i_ma"], vbat_mv=k["vbat_mv"],
                       byte_offset_fraction=k["byte_offset_fraction"],
                       n_bb_events=len(k["bb_events"])))(parse_kill(f))
    for f in ("cmp_clone_step90_raw.bin", "cmp_clone_step_raw.bin")}

# ---- low ------------------------------------------------------------
low = {}
for fw, name in [("clone", "cmp_clone_low_trace.csv"),
                 ("am32", "cmp_am32_low_trace.csv")]:
    rows = load_trace(name, fw)
    segs = plateau_segments(rows, min_count=800, tol=3)
    duties = sorted({s["duty"] for s in segs})
    pct_of = {d: p for d, p in zip(duties, (3, 4, 5))}
    per = {}
    for s in segs:
        pct = pct_of[s["duty"]]
        per.setdefault(pct, []).extend(s["recs"])
    out = []
    for pct in sorted(per):
        rr = per[pct]
        run = [r["zt"] for r in rr
               if not r["old"] and 20 <= r["zt"] <= 20000]
        old_n = sum(r["old"] for r in rr)
        fe = [1e6 / (6 * z) for z in run]
        out.append(dict(
            throttle=pct, n_records=len(rr),
            fe_med_hz=round(statistics.median(fe), 1) if fe else None,
            fe_sigma_hz=round(statistics.pstdev(fe), 1)
            if len(fe) > 1 else None,
            old_routine_fraction=round(old_n / len(rr), 3)))
    low[fw] = dict(rows=rows, per=out)
    metrics.setdefault("low", {})[fw] = out

# ---- jitter at 60% (ladder up-leg rung) -----------------------------
jit = {}
for fw in ("clone", "am32"):
    seg60 = next(s for s in lad[fw]["up"]
                 if lad[fw]["pct_of"][s["duty"]] == 60)
    st, dev = deviations_pct(seg60["recs"])
    per_sector = {}
    for s in range(1, 7):
        d = [v for stp, v in zip(st, dev) if stp == s]
        if d:
            per_sector[s] = dict(n=len(d),
                                 mean_pct=round(statistics.mean(d), 3),
                                 sigma_pct=round(statistics.pstdev(d), 3))
    jit[fw] = dict(dev=dev, steps=st, per_sector=per_sector,
                   overall_sigma_pct=round(statistics.pstdev(dev), 3))
    metrics.setdefault("jitter_60pct", {})[fw] = dict(
        per_sector=per_sector,
        overall_sigma_pct=jit[fw]["overall_sigma_pct"],
        n=len(dev))
metrics["jitter_60pct"]["note"] = (
    "the deviation distribution is BIMODAL (~+/-7%) for BOTH"
    " firmwares — a systematic alternating sector-period pattern"
    " (mechanical/sensing asymmetry of this motor), not gaussian"
    " noise; sigma mostly measures the mode separation")

# ---- starts (session-reported) --------------------------------------
metrics["starts"] = dict(
    clone=dict(success="8/8 at 20%", mean_detect_s=0.5,
               note="session-reported (earlier bench session; not"
                    " re-derived from these captures)"),
    am32="not measured (start-count script protocol is clone-specific)")

# ================================================================ figs

def style_leg(ax):
    h, _ = ax.get_legend_handles_labels()
    if h:
        ax.legend(loc="best", fontsize=9)


def plot_broken(ax, ts, fs, gap_s=0.25, **kw):
    """Line plot that breaks at time gaps (start attempts, batch
    decimation) instead of drawing a false connecting segment."""
    xs, ys = [], []
    prev = None
    for t, f in zip(ts, fs):
        if prev is not None and t - prev > gap_s:
            xs.append(float("nan"))
            ys.append(float("nan"))
        xs.append(t)
        ys.append(f)
        prev = t
    ax.plot(xs, ys, **kw)


# fig_map
fig, axes = plt.subplots(1, 3, figsize=FIGSIZE, dpi=DPI)
panels = [("fe", "f_e (Hz)"), ("ma", "current (mA)"),
          ("mv", "vbat (mV)")]
for ax, (k, lbl) in zip(axes, panels):
    xc = sorted(map_c)
    xa = sorted(map_a)
    ax.plot(xc, [map_c[r][k] for r in xc], "-o", color=C_CLONE,
            ms=4, lw=2, label="clone")
    ax.plot(xa, [map_a[r][k] for r in xa], "-s", color=C_AM32,
            ms=4, lw=2, label="AM32")
    ax.set_xlabel("throttle (%)")
    ax.set_title(lbl)
    style_leg(ax)
fig.suptitle("Operating map — am32_clone vs AM32 (same bench)", y=0.99)
fig.tight_layout()
fig.savefig(OUT / "fig_map.png")
plt.close(fig)

# fig_ladder
fig, axes = plt.subplots(1, 3, figsize=FIGSIZE, dpi=DPI)
for fw, col in (("clone", C_CLONE), ("am32", C_AM32)):
    for leg, ls, mk in (("up", "-", "o"), ("down", "--", "^")):
        pr = [p for p in metrics["ladder"][fw]["per_rung"]
              if p["leg"] == leg]
        if not pr:
            continue
        x = [p["throttle"] for p in pr]
        axes[0].plot(x, [p["med_period_us"] for p in pr], ls,
                     marker=mk, ms=4, lw=1.8, color=col,
                     label=f"{'clone' if fw == 'clone' else 'AM32'}"
                           f" {leg}")
        axes[1].plot(x, [p["exc12_per_1k"] for p in pr], ls,
                     marker=mk, ms=4, lw=1.8, color=col)
        axes[2].plot(x, [p["worst_pct"] for p in pr], ls,
                     marker=mk, ms=4, lw=1.8, color=col)
axes[0].set_title("median electrical period (µs)")
axes[1].set_title(">12.5% excursions per 1k records")
axes[2].set_title("worst excursion (%)")
for ax in axes:
    ax.set_xlabel("throttle (%)")
axes[1].set_yscale("symlog", linthresh=1)
style_leg(axes[0])
fig.suptitle("Ladder 10→100→10% — per-rung stability"
             " (AM32 trace ends at 90% up-leg)", y=0.99)
fig.tight_layout()
fig.savefig(OUT / "fig_ladder.png")
plt.close(fig)


def t_shift(fwtag):
    """x=0 at the first 10->60 duty movement."""
    d = step_data[fwtag]
    key = "clone" if fwtag.startswith("clone") else "am32"
    prof = fwtag.split("_")[1]
    tr = steps[f"{key}_step{prof}"]["transitions"].get("10_to_60")
    return tr["t_step_s"] if tr and tr.get("t_step_s") else 0.0


# fig_step
fig, axes = plt.subplots(2, 1, figsize=(1200 / 110, 760 / 110), dpi=DPI,
                         sharex=False)
for ax, prof in zip(axes, ("90", "100")):
    for fw, col in (("clone", C_CLONE), ("am32", C_AM32)):
        d = step_data[f"{fw}_{prof}"]
        t0 = t_shift(f"{fw}_{prof}")
        plot_broken(ax, [t - t0 for t in d["ts"]], d["fs"], color=col,
                    lw=1.2, label="clone" if fw == "clone" else "AM32")
    ax.set_ylabel("f_e (Hz)")
    ax.set_title(f"profile 10→60→10→{prof}→10%  (t=0 at 10→60 step)")
    ax.set_xlim(-8.5, 14.5)
    style_leg(ax)
    d = step_data[f"clone_{prof}"]
    t0 = t_shift(f"clone_{prof}")
    tk = d["rows"][-1]["t"] - t0
    ax.axvline(tk, color=C_CLONE, ls=":", lw=1.5)
    ax.annotate("vbat-floor kill", xy=(tk, ax.get_ylim()[1] * 0.9),
                xytext=(tk - 4.5, ax.get_ylim()[1] * 0.92),
                color=C_CLONE, fontsize=9,
                arrowprops=dict(arrowstyle="->", color=C_CLONE))
axes[1].set_xlabel("time (s)")
fig.suptitle("Step response — clone killed on every 10→90/100 slam;"
             " AM32 rides it", y=0.995)
fig.tight_layout()
fig.savefig(OUT / "fig_step.png")
plt.close(fig)

# fig_step_zoom
zoom_defs = [("10_to_60", "10→60 rise", "90"),
             ("60_to_10", "60→10 fall", "90"),
             ("hi_rise", "10→90/100 rise", None),
             ("hi_fall", "90/100→10 fall", None)]
fig, axes = plt.subplots(2, 2, figsize=(1200 / 110, 760 / 110), dpi=DPI)
axes = axes.ravel()
for ax, (key, title, _) in zip(axes, zoom_defs):
    for fw, col in (("clone", C_CLONE), ("am32", C_AM32)):
        prof = "90" if key in ("10_to_60", "60_to_10") else None
        # hi transitions: clone uses its 90 profile (both die);
        # am32 uses 90 profile too for like-for-like
        prof = prof or "90"
        skey = f"{fw}_step{prof}"
        tr_all = steps[skey]["transitions"]
        if key == "hi_rise":
            k = next((x for x in tr_all if x.startswith("10_to_")
                      and x not in ("10_to_60",)), None)
        elif key == "hi_fall":
            k = next((x for x in tr_all if x.endswith("_to_10")
                      and x != "60_to_10"), None)
        else:
            k = key
        tr = tr_all.get(k) if k else None
        if not tr or not tr.get("t_step_s"):
            continue
        t0 = tr["t_step_s"]
        d = step_data[f"{fw}_{prof}"]
        span = (-0.1, 3.0)
        pts = [(1e3 * (t - t0), f) for t, f in zip(d["ts"], d["fs"])
               if span[0] <= t - t0 <= span[1]]
        if pts:
            ax.plot([p[0] for p in pts], [p[1] for p in pts],
                    color=col, lw=1.4,
                    label="clone" if fw == "clone" else "AM32")
            if key == "hi_rise" and fw == "clone":
                ax.plot(pts[-1][0], pts[-1][1], "x", color=col, ms=9,
                        mew=2.2)
                ax.annotate("killed", xy=pts[-1],
                            xytext=(pts[-1][0] + 60, pts[-1][1]),
                            color=col, fontsize=9)
    ax.set_title(title)
    ax.set_xlabel("ms since first duty movement")
    ax.set_ylabel("f_e (Hz)")
    style_leg(ax)
axes[2].text(0.02, 0.02, "clone: killed mid-ramp (no plateau)",
             transform=axes[2].transAxes, fontsize=8, color=C_CLONE)
axes[3].set_xticks([])
axes[3].set_yticks([])
axes[3].grid(False)
axes[3].text(0.5, 0.5, "not captured:\nclone dead before the fall"
             " (vbat-floor kill at 90/100%);\nAM32 capture ends"
             " during the high dwell", ha="center", va="center",
             transform=axes[3].transAxes, fontsize=10, color=MUTED)
fig.suptitle("Zoomed transitions (90-top profile)", y=0.995)
fig.tight_layout()
fig.savefig(OUT / "fig_step_zoom.png")
plt.close(fig)

# fig_jitter
fig, axes = plt.subplots(1, 2, figsize=FIGSIZE, dpi=DPI)
w = 0.35
for i, (fw, col) in enumerate((("clone", C_CLONE), ("am32", C_AM32))):
    ps = jit[fw]["per_sector"]
    xs = [s + (i - 0.5) * w for s in sorted(ps)]
    means = [ps[s]["mean_pct"] for s in sorted(ps)]
    sig = [ps[s]["sigma_pct"] for s in sorted(ps)]
    axes[0].bar(xs, sig, width=w, color=col,
                label="clone" if fw == "clone" else "AM32")
    hist_range = (-20, 20)
    axes[1].hist(jit[fw]["dev"], bins=100, range=hist_range,
                 density=True, histtype="step", lw=1.8, color=col,
                 label=("clone σ=%.2f%%" % jit[fw]["overall_sigma_pct"])
                 if fw == "clone" else
                 ("AM32 σ=%.2f%%" % jit[fw]["overall_sigma_pct"]))
axes[0].set_xlabel("sector (step 1..6)")
axes[0].set_ylabel("σ of deviation (%)")
axes[0].set_title("per-sector zt jitter σ vs 6-deep rolling mean")
axes[1].set_xlabel("deviation from rolling mean (%)")
axes[1].set_ylabel("density")
axes[1].set_title("deviation distribution")
for ax in axes:
    style_leg(ax)
fig.suptitle("Commutation jitter at the 60% plateau (ladder up-leg)",
             y=0.99)
fig.tight_layout()
fig.savefig(OUT / "fig_jitter.png")
plt.close(fig)

# fig_low  (t=0 aligned at the first 3%->4% duty movement per fw)
fig, ax = plt.subplots(figsize=FIGSIZE, dpi=DPI)
for fw, col in (("clone", C_CLONE), ("am32", C_AM32)):
    rows = low[fw]["rows"]
    segs3 = plateau_segments(rows, min_count=800, tol=3)
    t0 = segs3[0]["t1"] if segs3 else 0.0
    ts, fs = fe_series(rows, smooth=9)
    ts = [t - t0 for t in ts]
    lbl = "clone" if fw == "clone" else "AM32"
    plot_broken(ax, ts, fs, color=col, lw=0.7, alpha=0.35)
    tsm, fsm = fe_series(rows, smooth=101)
    plot_broken(ax, [t - t0 for t in tsm], fsm, color=col, lw=2.2,
                label=lbl)
    # old_routine density strip
    to = [r["t"] - t0 for r in rows if r["old"]]
    ax.plot(to, [12 if fw == "clone" else 6] * len(to), "|",
            color=col, ms=6, alpha=0.25)
ax.text(0.01, 0.97, "bottom tick strips = old_routine (polling-mode)"
        " records — AM32 is 100% polling at 3% (no running-mode f_e"
        " at all); clone re-enters running mode even at 3%",
        transform=ax.transAxes, fontsize=8, color=MUTED, va="top")
ax.set_xlabel("time since end of first 3% dwell (s)")
ax.set_ylabel("f_e (Hz)")
ax.set_title("Low-throttle profile 3→4→5→4→3% — running-mode f_e"
             " (thin = 9-median, bold = 101-median)")
style_leg(ax)
fig.tight_layout()
fig.savefig(OUT / "fig_low.png")
plt.close(fig)

# ---- write metrics --------------------------------------------------
with open(OUT / "metrics.json", "w") as f:
    json.dump(metrics, f, indent=1)

print("wrote", OUT / "metrics.json")
for p in sorted(OUT.glob("fig_*.png")):
    print(f"  {p.name}  {p.stat().st_size}")
