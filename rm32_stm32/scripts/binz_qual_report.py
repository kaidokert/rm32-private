#!/usr/bin/env python3
"""Build the rm32-on-G071 (binz) qualification report from captures.

    python scripts/binz_qual_report.py --out <path.html> --wcet <wcet.txt>

Every table value is computed here from a named capture in captures/binz/
(profile_*.csv/json from binz_profile.py, spin_*/coast_*/restart_*/inject_*/
step_* from the stage-7 runs); the provenance section lists each file.
"""

from __future__ import annotations

import argparse
import base64
import csv
import glob
import html
import io
import json
import pathlib
import re
import statistics

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

CAP = pathlib.Path("captures/binz")
REPORT_IMAGE = "4064aef7"
STAGE7_IMAGES = {  # stage-7 hold captures -> image (BINZ_BRINGUP.md stage 7)
    "spin_20261006_183624": "9403f06b", "spin_20261006_183945": "9403f06b",
    "spin_20261006_184309": "9403f06b", "spin_20261006_184642": "9403f06b",
    "spin_20261006_185707": "7375b4c3", "spin_20261006_190045": "7375b4c3",
    "spin_20261006_190425": "7375b4c3", "spin_20261006_190811": "7375b4c3",
    "spin_20261006_191801": "765fcf9a", "spin_20261006_192202": "765fcf9a",
    "spin_20261006_192611": "765fcf9a", "spin_20261006_193015": "765fcf9a",
}
PHASE = {1: "C rising", 2: "A falling", 3: "B rising", 4: "C falling", 5: "A rising", 6: "B falling"}
SOURCES: list[str] = []

# Figure palette (light figures on a framed card; legible in both themes).
INK, MUTED, GRID = "#1d2733", "#5d6b7a", "#dde3ea"
C_SPD, C_CUR, C_BUS, C_DUTY, C_KILL = "#1f6fb2", "#c4522a", "#2f8f5b", "#7a5bb5", "#b3261e"
plt.rcParams.update({
    "font.family": "DejaVu Sans", "font.size": 9, "axes.edgecolor": MUTED,
    "axes.labelcolor": INK, "xtick.color": MUTED, "ytick.color": MUTED,
    "axes.grid": True, "grid.color": GRID, "grid.linewidth": 0.6,
    "axes.spines.top": False, "axes.spines.right": False, "figure.dpi": 110,
})


def src(path: str | pathlib.Path) -> str:
    p = str(pathlib.Path(path).as_posix())
    if p not in SOURCES:
        SOURCES.append(p)
    return p


def png(fig) -> str:
    buf = io.BytesIO()
    fig.savefig(buf, format="png", bbox_inches="tight", facecolor="white")
    plt.close(fig)
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def lines(path) -> list[str]:
    return pathlib.Path(path).read_bytes().decode("latin-1").replace("\r", "").splitlines()


# ---------------------------------------------------------------- profiles
class Profile:
    def __init__(self, name: str):
        base = sorted(glob.glob(str(CAP / f"profile_{name}_*.json")))[-1][:-5]
        self.name = name
        self.meta = json.loads(pathlib.Path(base + ".json").read_text())
        self.rows = list(csv.DictReader(open(base + ".csv")))
        src(base + ".csv"), src(base + ".json"), src(base + ".txt")
        self.t = [float(r["t_s"]) for r in self.rows]
        self.duty = [int(r["duty"]) for r in self.rows]
        self.ehz = [float(r["ehz"]) for r in self.rows]
        self.ma = [float(r["ma"]) for r in self.rows]
        self.mv = [float(r["mv"]) for r in self.rows]
        self.old = [int(r["old_routine"]) for r in self.rows]
        self.segs = self.meta["segments"]
        self.r = self.meta.get("r") or {}
        self.kill = self.meta.get("kill")
        self.base = base

    def seg_end(self, i: int) -> float:
        return self.segs[i + 1]["t_start"] if i + 1 < len(self.segs) else self.meta.get("t_stop", self.t[-1])

    def window(self, t0: float, t1: float) -> dict[str, float]:
        idx = [k for k, t in enumerate(self.t) if t0 <= t < t1]
        if not idx:
            return {}
        m = lambda a: statistics.fmean(a[k] for k in idx)  # noqa: E731
        return {"duty": m(self.duty), "ehz": m(self.ehz), "ma": m(self.ma), "mv": m(self.mv),
                "old": m(self.old), "n": len(idx)}

    def dwell_stats(self, tail: float = 3.0) -> list[dict]:
        out = []
        for i, s in enumerate(self.segs):
            t1 = self.seg_end(i)
            w = self.window(t1 - tail, t1 - 0.1)
            out.append({"throttle": s["throttle"], **w})
        return out


# ---------------------------------------------------------------- holds
def hold_capture(path: str) -> dict:
    L = lines(path)
    r = [ln for ln in L if ln.startswith("r duty=")]
    hs = [ln for ln in L if re.match(r"^h[1-6] n=", ln)][-6:]
    hx = [ln for ln in L if ln.startswith("hx ")]
    out = {"file": src(path)}
    if r:
        out.update({k: int(v) for k, v in re.findall(r"(\w+)=(-?\d+)", r[-1])})
    sec = {}
    for ln in hs:
        m = re.match(r"^h([1-6]) n=(\d+) dev_x6=(-?\d+) b=([\d,]+)$", ln)
        b = [int(x) for x in m.group(4).split(",")]
        n = sum(b)
        sec[int(m.group(1))] = {"n": n, "ge15": 100 * b[15] / n if n else 0, "mean_us": int(m.group(3)) / 12,
                                "bins": b}
    out["sectors"] = sec
    if hx:
        out["excursions"] = int(re.search(r"excursions=(\d+)", hx[-1]).group(1))
    return out


# ---------------------------------------------------------------- figures
def fig_map(lo: Profile, hi: Profile):
    d = lo.dwell_stats() + hi.dwell_stats()
    thr = [x["throttle"] for x in d]
    fig, ax = plt.subplots(3, 1, figsize=(8.2, 7.2), sharex=True)
    ax[0].plot(thr, [x["ehz"] for x in d], "o-", color=C_SPD, ms=4)
    ax[0].set_ylabel("electrical speed (eHz)")
    a2 = ax[0].twinx()
    a2.plot(thr, [x["duty"] / 20 for x in d], "s--", color=C_DUTY, ms=3, lw=1)
    a2.set_ylabel("applied duty (%)", color=C_DUTY)
    a2.spines["right"].set_visible(True)
    a2.grid(False)
    ax[1].plot(thr, [x["ma"] / 1000 for x in d], "o-", color=C_CUR, ms=4)
    ax[1].set_ylabel("bus current (A)")
    ax[2].plot(thr, [x["mv"] / 1000 for x in d], "o-", color=C_BUS, ms=4)
    ax[2].set_ylabel("bus voltage (V)")
    ax[2].set_xlabel("throttle command (%)")
    ax[2].set_xticks(range(5, 101, 5))
    return png(fig), d


def series(ax3, p: Profile, t_off: float = 0.0, label_kill: bool = True):
    t = [x - t_off for x in p.t]
    ax3[0].plot(t, p.ehz, color=C_SPD, lw=1.1)
    a2 = ax3[0].twinx()
    a2.plot(t, [x / 20 for x in p.duty], color=C_DUTY, lw=0.9, ls="--")
    a2.set_ylabel("duty %", color=C_DUTY)
    a2.set_ylim(0, 105)
    a2.grid(False)
    a2.spines["right"].set_visible(True)
    ax3[1].plot(t, [x / 1000 for x in p.ma], color=C_CUR, lw=1.0)
    ax3[2].plot(t, [x / 1000 for x in p.mv], color=C_BUS, lw=1.0)
    ax3[0].set_ylabel("eHz")
    ax3[1].set_ylabel("A (16 ms avg)")
    ax3[2].set_ylabel("bus V")
    if p.kill and label_kill:
        tk = p.t[-1] - t_off
        for a in ax3:
            a.axvline(tk, color=C_KILL, lw=1.2, ls=":")
        ax3[0].annotate("guard kill", (tk, max(p.ehz) * 0.92), color=C_KILL, ha="right", fontsize=8)


def fig_ladder(p: Profile):
    fig, ax = plt.subplots(3, 1, figsize=(8.6, 6.8), sharex=True)
    series(ax, p)
    for i, s in enumerate(p.segs):
        ax[0].text(s["t_start"] + 0.3, 3750, f"{s['throttle']}", fontsize=7, color=MUTED)
    ax[0].set_ylim(0, 3900)
    ax[2].set_xlabel("time from first rung (s), 50 ms samples")
    return png(fig)


def fig_ladder_updown(p: Profile):
    d = p.dwell_stats()
    up, down = d[:10], d[9:]
    fig, ax = plt.subplots(1, 2, figsize=(8.6, 3.2))
    ax[0].plot([x["throttle"] for x in up], [x["ehz"] for x in up], "o-", color=C_SPD, label="rising")
    ax[0].plot([x["throttle"] for x in down], [x["ehz"] for x in down], "s--", color=C_DUTY, label="falling")
    ax[0].set_xlabel("throttle (%)")
    ax[0].set_ylabel("eHz")
    ax[0].legend(frameon=False)
    ax[1].plot([x["throttle"] for x in up], [x["ma"] / 1000 for x in up], "o-", color=C_CUR, label="rising")
    ax[1].plot([x["throttle"] for x in down], [x["ma"] / 1000 for x in down], "s--", color=C_DUTY, label="falling")
    ax[1].set_xlabel("throttle (%)")
    ax[1].set_ylabel("A")
    ax[1].legend(frameon=False)
    return png(fig), d


def fig_steps(profiles: list[tuple[str, Profile]]):
    fig, ax = plt.subplots(3, len(profiles), figsize=(3.0 * len(profiles), 6.4), sharey="row")
    for j, (title, p) in enumerate(profiles):
        col = [ax[0][j], ax[1][j], ax[2][j]]
        series(col, p)
        ax[0][j].set_title(title, fontsize=9, color=INK)
        ax[2][j].set_xlabel("s (10 ms samples)")
        if j:
            for a in col:
                a.set_ylabel("")
    return png(fig)


def transitions(p: Profile) -> list[dict]:
    out = []
    for i in range(1, len(p.segs)):
        a, b = p.segs[i - 1], p.segs[i]
        t_cmd = b["t_start"]
        t_end = p.seg_end(i)
        # the first duty change after the command marks the response start
        k0 = next((k for k, t in enumerate(p.t) if t >= t_cmd - 0.05 and p.duty[k] != p.duty[max(k - 1, 0)]), None)
        if k0 is None:
            continue
        ts = p.t[k0]
        fin = p.window(t_end - 1.0, t_end - 0.05) if t_end - ts > 1.2 else {}
        start = p.window(ts - 0.5, ts)
        # a killed run's last sample is the kill instant itself: include it
        idx = [k for k, t in enumerate(p.t) if ts <= t < t_end or (p.kill and t == p.t[-1] and t >= ts)]
        pk = max(p.ma[k] for k in idx) if idx else 0
        vmin = min(p.mv[k] for k in idx) if idx else 0
        settle = None
        if fin and start:
            target = fin["ehz"]
            band = 0.05 * target
            last_out = None
            for k in idx:
                if abs(p.ehz[k] - target) > band:
                    last_out = p.t[k]
            settle = (last_out - ts) if last_out is not None else 0.0
        out.append({"from": a["throttle"], "to": b["throttle"], "start_ehz": start.get("ehz"),
                    "final_ehz": fin.get("ehz"), "settle_s": settle, "peak_ma": pk, "min_mv": vmin,
                    "killed": bool(p.kill) and i == len(p.segs) - 1 or (bool(p.kill) and not fin)})
    return out


def fig_lowend(p: Profile, lost: list[dict]):
    fig = plt.figure(figsize=(8.6, 5.2))
    gs = fig.add_gridspec(3, 3)
    ax = [fig.add_subplot(gs[0, :2]), fig.add_subplot(gs[1, :2]), fig.add_subplot(gs[2, :2])]
    series(ax, p)
    for s in p.segs:
        ax[0].text(s["t_start"] + 0.2, max(p.ehz) * 0.9 + 10, f"{s['throttle']}%", fontsize=7, color=MUTED)
    ax[2].set_xlabel("s (50 ms samples)")
    axb = fig.add_subplot(gs[:, 2])
    axb.bar([f"{x['thr']}%" for x in lost], [x["gaplost"] for x in lost], color=C_KILL)
    axb.set_title("control ticks lost per 10 s", fontsize=9, color=INK)
    for k, x in enumerate(lost):
        axb.text(k, x["gaplost"], str(x["gaplost"]), ha="center", va="bottom", fontsize=8)
    fig.tight_layout()
    return png(fig)


def fig_starts(starts: list[Profile], direct: list[Profile]):
    fig, ax = plt.subplots(2, 2, figsize=(8.6, 5.0), sharex="col")
    for k, p in enumerate(starts):
        ax[0][0].plot(p.t, p.ehz, lw=1.0, label=f"start {k + 1}")
        ax[1][0].plot(p.t, [x / 1000 for x in p.ma], lw=0.9)
    for k, p in enumerate(direct):
        ax[0][1].plot(p.t, p.ehz, lw=1.0, label=f"direct {k + 1}")
        ax[1][1].plot(p.t, [x / 1000 for x in p.ma], lw=0.9)
    ax[0][0].set_title("standard: sine ramp, walk-in 5-10 %, hold 10 %", fontsize=9, color=INK)
    ax[0][1].set_title("direct: sine ramp, then straight to 20 %", fontsize=9, color=INK)
    for a in (ax[0][0], ax[0][1]):
        a.set_ylabel("eHz")
        a.legend(frameon=False, fontsize=8)
    for a in (ax[1][0], ax[1][1]):
        a.set_ylabel("A (16 ms avg)")
        a.set_xlabel("s from the start of the sine ramp")
    return png(fig)


def fig_quality(holds: list[dict]):
    holds = [h for h in holds if h.get("sectors")]
    duty = [h["duty"] / 20 for h in holds]
    fig, ax = plt.subplots(1, 2, figsize=(8.6, 3.4))
    for s in range(1, 7):
        ax[0].plot(duty, [h["sectors"][s]["ge15"] for h in holds], "o-", ms=3, label=f"{s} {PHASE[s]}")
    ax[0].set_xlabel("applied duty (%)")
    ax[0].set_ylabel("intervals >= 15 us off the 6-step mean (%)")
    ax[0].legend(frameon=False, fontsize=7)
    top = holds[-1]
    x = list(range(16))
    for s in range(1, 7):
        b = top["sectors"][s]["bins"]
        n = sum(b)
        ax[1].plot(x, [100 * v / n for v in b], lw=1, label=str(s))
    ax[1].set_xlabel("|interval - 6-step mean| (us, last bin = 15+)")
    ax[1].set_ylabel("% of intervals")
    ax[1].set_title(f"per sector at {top['duty'] / 20:.0f} % duty", fontsize=9, color=INK)
    ax[1].legend(frameon=False, fontsize=7, ncol=2)
    return png(fig)


# ---------------------------------------------------------------- html
def esc(x) -> str:
    return html.escape(str(x))


def fmt(v, nd=0, dash="—"):
    if v is None:
        return dash
    return f"{v:,.{nd}f}"


def table(head: list[str], rows: list[list], cls: str = "") -> str:
    th = "".join(f"<th>{esc(h)}</th>" for h in head)
    tr = "".join("<tr>" + "".join(f"<td>{c}</td>" for c in r) + "</tr>" for r in rows)
    return f'<div class="tw"><table class="{cls}"><thead><tr>{th}</tr></thead><tbody>{tr}</tbody></table></div>'


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--wcet", required=True)
    a = ap.parse_args()

    lo, hi = Profile("map-lo"), Profile("map-hi")
    ladder = Profile("ladder")
    steps = {n: Profile(n) for n in ("step-60", "step-90", "step-100", "step-90-down", "step-100-down", "step-100-25")}
    lowend = Profile("lowend")
    slams = {n: Profile(n) for n in ("slam15-90", "slam15a-100", "slam5-100")}
    lost = []
    for t in (5, 10, 15):
        p = Profile(f"lost-{t}")
        lost.append({"thr": t, **{k: p.r.get(k) for k in ("duty", "gaplost", "gaplate", "gapmax", "comp_entries", "camps")}})
    starts = [Profile(f"start{k}") for k in (1, 2, 3)]
    direct = [Profile("start-direct20"), Profile("start-direct20-2"), Profile("start-direct20-3")]
    check = sorted(glob.glob(str(CAP / "profile_check_*.csv")))
    holds = [hold_capture(str(CAP / f"{n}.txt")) for n in STAGE7_IMAGES]
    coast100 = lines(src(CAP / "coast_20261006_193420.txt"))
    coast80 = lines(src(CAP / "coast_20261006_183259.txt"))
    wcet = pathlib.Path(a.wcet).read_text().splitlines()
    src(a.wcet)

    f_map, mapd = fig_map(lo, hi)
    f_lad = fig_ladder(ladder)
    f_ud, ladd = fig_ladder_updown(ladder)
    f_up = fig_steps([("10 → 60 → 10 %", steps["step-60"]), ("10 → 90 %", steps["step-90"]),
                      ("10 → 100 %", steps["step-100"])])
    f_dn = fig_steps([("… 90 → 10 %", steps["step-90-down"]), ("… 100 → 10 %", steps["step-100-down"]),
                      ("… 100 → 25 %", steps["step-100-25"])])
    f_low = fig_lowend(lowend, lost)
    f_slam = fig_steps([("10 → 90 %", slams["slam15-90"]), ("10 → 100 %", slams["slam15a-100"]),
                        ("5 → 100 %", slams["slam5-100"])])
    f_st = fig_starts(starts, direct)
    f_q = fig_quality(sorted(holds, key=lambda h: h.get("duty", 0)))

    # ---- derived numbers
    top = mapd[-1]
    m100 = [h for h in holds if h.get("duty") == 2000]
    trans = []
    for n, p in steps.items():
        for tr in transitions(p):
            trans.append((n, p, tr))

    def coast_line(L):
        for ln in L:
            if ln.startswith("coast iv_cyc"):
                pass
        rows = [ln for ln in L if ln.startswith("coast iv_cyc[")]
        iv = []
        for ln in rows[-4:]:
            iv += [int(x) for x in ln.split("]")[1].split()]
        used = [c / 64 for c in iv[1:9] if c]
        r = [ln for ln in L if ln.startswith("r duty=")][-1]
        ec = int(re.search(r"ecom10=(\d+)", r).group(1))
        return 1e6 / (2 * statistics.median(used)), 1e7 / ec

    c100, l100 = coast_line(coast100)
    c80, l80 = coast_line(coast80)

    # restarts / injects (stage 7, 765fcf9a)
    ev = []
    for f in sorted(glob.glob(str(CAP / "restart_20261006_19*.txt"))):
        L = lines(src(f))
        r = [ln for ln in L if ln.startswith("r duty=")][-1]
        d = {k: int(v) for k, v in re.findall(r"(\w+)=(-?\d+)", r)}
        ev.append(("restart, 300 ms coast, re-walk", f"{1e7 / d['ecom10']:.0f}", d["imean_ma"], d["dsy"], pathlib.Path(f).name))
    for f in sorted(glob.glob(str(CAP / "inject_20261006_20*.txt"))):
        L = lines(src(f))
        r = [ln for ln in L if ln.startswith("r duty=")][-1]
        d = {k: int(v) for k, v in re.findall(r"(\w+)=(-?\d+)", r)}
        ev.append(("injected skipped step, re-hold", f"{1e7 / d['ecom10']:.0f}", d["imean_ma"], d["dsy"], pathlib.Path(f).name))

    # ---- sections
    map_rows = [[f"{x['throttle']}", fmt(x["duty"] / 20, 1), fmt(x["ehz"]), fmt(x["ma"]), fmt(x["mv"]),
                 "polling" if x["old"] > 0.5 else "interrupt"] for x in mapd]
    q_rows = []
    for h in sorted(holds, key=lambda h: h.get("duty", 0)):
        if not h.get("sectors"):
            continue
        s = h["sectors"]
        q_rows.append([fmt(h["duty"] / 20, 1), f"{1e7 / h['ecom10']:.0f}", fmt(h["imean_ma"]), h.get("dsy"),
                       h.get("excursions", "—"), fmt(s[2]["ge15"], 1), fmt(s[4]["ge15"], 1),
                       f"{h.get('gaplost', '—')}", STAGE7_IMAGES[pathlib.Path(h['file']).stem]])
    lad_rows = []
    for i in range(10):
        u, d = ladd[i], ladd[18 - i]
        lad_rows.append([f"{u['throttle']}", fmt(u["ehz"]), fmt(d["ehz"]),
                         fmt(100 * (d["ehz"] - u["ehz"]) / u["ehz"], 2), fmt(u["ma"]), fmt(d["ma"]), fmt(u["mv"]), fmt(d["mv"])])
    tr_rows = []
    for n, p, tr in trans:
        tr_rows.append([f"{tr['from']} → {tr['to']} %", fmt(tr["start_ehz"]), fmt(tr["final_ehz"]),
                        fmt(tr["settle_s"] * 1000 if tr["settle_s"] is not None else None),
                        fmt(tr["peak_ma"]), fmt(tr["min_mv"]),
                        '<span class="pill bad">guard kill</span>' if (p.kill and tr["final_ehz"] is None) else '<span class="pill ok">held</span>',
                        esc(n)])
    slam_rows = []
    for n, p in slams.items():
        for tr in transitions(p):
            if tr["to"] in (90, 100):
                slam_rows.append([f"{tr['from']} → {tr['to']} %", fmt(tr["start_ehz"]), fmt(tr["final_ehz"]),
                                  fmt(tr["settle_s"] * 1000 if tr["settle_s"] is not None else None),
                                  fmt(tr["peak_ma"]), fmt(tr["min_mv"]), p.r.get("dsy"),
                                  {"slam15-90": "15 % / 12 A", "slam15a-100": "17.5 % / 15 A",
                                   "slam5-100": "17.5 % / 15 A"}[n], esc(n)])
    lowd = lowend.dwell_stats(2.5)
    low_rows = [[f"{x['throttle']}", fmt(x["duty"]), "sine stepper (open loop)" if x["duty"] == 0 else
                 ("polling" if x["old"] > 0.5 else "interrupt"), fmt(x["ma"])] for x in lowd]
    lost_rows = [[f"{x['thr']}", x["duty"], x["gaplost"], x["gaplate"], f"{x['gapmax'] / 64:.0f}",
                  fmt(x["comp_entries"] / 10), fmt(x["camps"] / 10)] for x in lost]
    st_rows = []
    for k, p in enumerate(starts + direct):
        bl = next((t for t, d in zip(p.t, p.duty) if d > 0), None)
        lock = next((t for t, d, o, e in zip(p.t, p.duty, p.old, p.ehz) if d > 0 and o == 0 and e > 200), None)
        st_rows.append([("standard " + str(k + 1)) if k < 3 else ("direct " + str(k - 2)), fmt(bl, 2), fmt(lock, 2),
                        fmt(max(p.ma)), p.r.get("dsy"), esc(pathlib.Path(p.base).name)])
    wcet_rows = []
    for ln in wcet:
        m = re.match(r"^(TIM14|ADC_COMP|TIM6_DAC_LPTIM1|DMA1_CHANNEL1)\s+(\d)\s+(\d+)\s+(\S+)\s+(\S+)\s+(\d+)\s+(\d+)\s+([\d.]+)", ln)
        if m:
            g = m.groups()
            wcet_rows.append([{"TIM14": "commutation (TIM14)", "ADC_COMP": "comparator (ADC_COMP)",
                               "TIM6_DAC_LPTIM1": "control tick (TIM6)", "DMA1_CHANNEL1": "ADC DMA"}[g[0]],
                              g[1], g[2], g[3], g[4], g[7]])
    budget = [ln for ln in wcet if ln.startswith("rung ")]

    kills = {n: p.meta.get("kill") for n, p in steps.items() if p.kill}
    kill90 = re.search(r"vbat_mv=(\d+) i_ma=(\d+) rest_mv=(\d+) ref_mv=(\d+)", kills.get("step-90", "")) if kills.get("step-90") else None
    kill100 = re.search(r"vbat_mv=(\d+) i_ma=(\d+) rest_mv=(\d+) ref_mv=(\d+)", kills.get("step-100", "")) if kills.get("step-100") else None

    def killtxt(m):
        if not m:
            return "—"
        v, i, rest, ref = (int(x) for x in m.groups())
        return f"bus {v / 1000:.2f} V vs running reference {ref / 1000:.2f} V ({100 * (ref - v) / ref:.1f} % drop), {i / 1000:.1f} A"

    sources = "".join(f"<li><code>{esc(s)}</code></li>" for s in SOURCES)
    h100 = m100[-1] if m100 else {}

    page = f"""<title>rm32 G071 Qualification</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans+Condensed:wght@500;600&family=IBM+Plex+Sans:ital,wght@0,400;0,500;0,600;1,400&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
/* Layout: one 880px reading column of findings, tables and framed figures; a bench-log register. */
:root {{
  --bg: #f7f8f6; --surface: #ffffff; --fg: #1d2733; --muted: #5d6b7a; --rule: #d9dfe4;
  --accent: #1f6fb2; --ok: #2f7d4f; --bad: #b3261e; --warn: #a76a0c; --code: #eef1f0;
  --display: "IBM Plex Sans Condensed", "Arial Narrow", system-ui, sans-serif;
  --body: "IBM Plex Sans", system-ui, -apple-system, "Segoe UI", sans-serif;
  --mono: "IBM Plex Mono", ui-monospace, Consolas, monospace;
}}
@media (prefers-color-scheme: dark) {{ :root:not([data-theme="light"]) {{
  --bg: #12171c; --surface: #1a2027; --fg: #dde4ea; --muted: #93a1ae; --rule: #2c353f;
  --accent: #6aa9e0; --ok: #5cbf86; --bad: #f08a80; --warn: #e0ac52; --code: #202831; color-scheme: dark }} }}
:root[data-theme="dark"] {{
  --bg: #12171c; --surface: #1a2027; --fg: #dde4ea; --muted: #93a1ae; --rule: #2c353f;
  --accent: #6aa9e0; --ok: #5cbf86; --bad: #f08a80; --warn: #e0ac52; --code: #202831; color-scheme: dark }}
body {{ background: var(--bg); color: var(--fg); font: 15px/1.6 var(--body); padding-inline: 16px; padding-block: 8px 80px; }}
main {{ max-width: 880px; margin: 0 auto; }}
h1 {{ font: 600 30px/1.15 var(--display); margin: 28px 0 6px; text-wrap: balance; letter-spacing: -.01em; }}
h2 {{ font: 600 20px/1.25 var(--display); margin: 44px 0 10px; padding-top: 14px; border-top: 1px solid var(--rule); text-wrap: balance; }}
h3 {{ font: 600 15.5px/1.3 var(--body); margin: 22px 0 6px; }}
p, li {{ max-width: 68ch; }}
.sub {{ color: var(--muted); font-size: 13.5px; margin: 0 0 6px; }}
.eyebrow {{ font: 500 11.5px/1 var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--accent); margin-top: 22px; }}
.verdict {{ background: var(--surface); border: 1px solid var(--rule); border-radius: 6px; padding: 14px 18px; margin: 18px 0; }}
.verdict ul {{ margin: 6px 0 2px; padding-left: 20px; }} .verdict li {{ margin: 6px 0; }}
figure {{ margin: 16px 0; }}
figure img {{ width: 100%; height: auto; border: 1px solid var(--rule); border-radius: 4px; background: #fff; }}
figcaption {{ font-size: 13px; color: var(--muted); margin-top: 6px; max-width: 80ch; }}
.tw {{ overflow-x: auto; margin: 12px 0; }}
table {{ border-collapse: collapse; font-size: 13.5px; font-variant-numeric: tabular-nums; min-width: 100%; }}
th, td {{ padding: 5px 10px; text-align: right; border-bottom: 1px solid var(--rule); white-space: nowrap; }}
th {{ font: 500 12px/1.3 var(--body); color: var(--muted); }}
th:first-child, td:first-child {{ text-align: left; }}
td:last-child {{ color: var(--muted); }}
code {{ font: 12.5px var(--mono); background: var(--code); padding: 1px 5px; border-radius: 3px; overflow-wrap: anywhere; }}
.pill {{ font: 500 11.5px/1 var(--mono); padding: 3px 7px; border-radius: 10px; border: 1px solid currentColor; }}
.pill.ok {{ color: var(--ok); }} .pill.bad {{ color: var(--bad); }}
.note {{ font-size: 13.5px; color: var(--muted); border-left: 3px solid var(--rule); padding: 2px 0 2px 12px; margin: 12px 0; }}
.prov li {{ font-size: 12.5px; }}
a {{ color: var(--accent); }}
a:focus-visible {{ outline: 2px solid var(--accent); outline-offset: 2px; }}
</style>
<main>
<p class="eyebrow">rm32 · STM32G071 · qualification report</p>
<h1>rm32 on the G071 bench</h1>
<p class="sub">NUCLEO-G071RB + BOOSTXL-DRV8304H, 3S battery, 48 kHz fixed carrier · report image <code>{REPORT_IMAGE}</code> · 2026-10-06</p>
<p>rm32 is a Rust transliteration of AM32 (the open-source drone ESC firmware). This report qualifies its G071 port on one bench motor across the whole throttle range, using a time-series recorder that samples in RAM and is read out only after each run, because any UART traffic while driving disturbs the comparator.</p>

<div class="verdict"><strong>Summary</strong>
<ul>
<li><strong>Full range, locked.</strong> Every operating-map dwell from 5 to 100 % throttle and every ladder rung held with zero desyncs. 100 % = duty 2000/2000, {fmt(top['ehz'])} eHz at {fmt(top['ma'] / 1000, 2)} A; the coasting rotor confirms {c100:.0f} eHz (loop reads {100 * (l100 / c100 - 1):+.1f} %).</li>
<li><strong>Clean steady state at speed.</strong> At 100 % the six commutation intervals sit within a few microseconds of their mean, with no excursions and no lost control ticks. The one systematic timing bias, an early accept on phase A falling, appears on firmware50 on the same board too, and fell below 1 % after the commutation-path cut.</li>
<li><strong>Down-steps and moderate up-steps ride through.</strong> 10→60→10, 90→10, 100→10 and 100→25 % all held with zero desyncs.</li>
<li><strong>Full slams pass once the bench guard has room.</strong> At the original 10 % sag / 12 A surge settings, 10→90 and 10→100 % are killed by the guard on acceleration inrush, with the loop locked. With sag 17.5 % and surge 15 A, 10→90, 10→100 and 5→100 % all hold: inrush peaks 10.0–13.0 A, bus down 13–16 %, full speed within 90–500 ms (section 4b).</li>
<li><strong>The low end has two weak spots.</strong> At 5 % throttle the control tick loses about 920 ticks per second to comparator camping (worst gap 0.64 ms); and a start straight into 20 % failed to lock once in four tries.</li>
</ul></div>

<h2>1 · Operating map</h2>
<p>Instant setpoint steps of 5 % throttle, 5 s dwells, recorded at 50 ms; each row is the mean of the last 3 s of its dwell. Two runs: 5–50 % and 55–100 %.</p>
<figure><img src="{f_map}" alt="Operating map: speed, duty, current and bus voltage against throttle"><figcaption>Speed rises almost linearly with duty up to ~90 % and bends slightly above it. Bus current follows roughly duty squared (prop load). The pack sags about 1 V at 7 A.</figcaption></figure>
{table(["throttle %", "duty %", "eHz", "mA", "bus mV", "mode"], map_rows)}

<h2>2 · Steady-state quality</h2>
<p>Quiet 15 s holds from the stage-7 climb on the battery, read from each hold's on-chip sector histogram (|interval − mean of the last six|, 1 µs bins) and post-run counters.</p>
<figure><img src="{f_q}" alt="Per-sector timing deviation against duty, and the 100 percent distribution"><figcaption>Left: the share of intervals at least 15 µs off the six-step mean, per sector (step number and the phase crossing it ends on). Sector 2 (phase A falling) and its long partner, sector 3, are the only ones that rise: 6–11 % at 80–87.5 % duty on the image before the commutation-path cut, under 1 % from the cut on (the two 87.5 % points), and 0 from 95 %. Right: the distribution at 100 %, where every sector stays within a few microseconds.</figcaption></figure>
{table(["duty %", "eHz", "mA", "desyncs", "excursions", "sector 2 ≥15 µs %", "sector 4 ≥15 µs %", "ticks lost", "image"], q_rows)}
<p class="note">An excursion is an interval at least 1.5× the six-step mean. The 80–85 % excursion counts are the sector-2 early accept pairs crossing that threshold, not a separate mechanism (stage 6 traces). Holds were run before the commutation-path cut on images <code>9403f06b</code>; the cut (<code>7375b4c3</code> on) halved control-tick overruns and shrank the sector-2/3 bias.</p>

<h2>3 · Ladder 10 → 100 → 10 %</h2>
<p>19 rungs of 10 % throttle, 5 s each, as instant steps, recorded at 50 ms. No desyncs, no kill.</p>
<figure><img src="{f_lad}" alt="Ladder time series of speed, duty, current and bus voltage"><figcaption>Rung numbers in % throttle along the top. Each up-step lands within about a second; each down-step brakes to the new speed with damped drive.</figcaption></figure>
<figure><img src="{f_ud}" alt="Ladder rising versus falling rungs"><figcaption>Rising and falling rungs land on the same operating point: speed within 0.8 %, current within 70 mA.</figcaption></figure>
{table(["throttle %", "eHz rising", "eHz falling", "Δ %", "mA rising", "mA falling", "bus mV rising", "bus mV falling"], lad_rows)}

<h2>4 · Step response</h2>
<p>Instant setpoint steps, recorded at 10 ms. The firmware's own duty ramp shapes the response (AM32 defaults for this target: 2 / 6 / 16 duty counts per 50 µs tick at startup / low / high speed). The big down-steps were reached through intermediate up-steps no larger than the 10→60 step that holds.</p>
<figure><img src="{f_up}" alt="Up-step responses"><figcaption>Up-steps from 10 %. 10→60→10 holds. 10→90 and 10→100 end at the dotted line: the 10 % bus-sag guard kills on the acceleration inrush.</figcaption></figure>
<figure><img src="{f_dn}" alt="Down-step responses"><figcaption>Down-steps from 90 and 100 %. All hold, with regenerative braking absorbed by the battery.</figcaption></figure>
{table(["transition", "eHz before", "eHz settled", "settle ms (±5 %)", "peak mA", "min bus mV", "result", "run"], tr_rows)}
<p class="note">10→90: {killtxt(kill90)}. 10→100: {killtxt(kill100)}. Settle time is from the first duty movement until speed last leaves a ±5 % band around the final second's mean.</p>

<h3>4b · Full slams with the bench guard opened up</h3>
<p>Operator follow-up (2026-10-07). The sag guard was raised to 15 %, then 17.5 % of the running bus reference, and the fast-current surge ceiling from 12 to 13, then 15 A; the 8 A sustained limit, 9 V floor and over-voltage stop are unchanged. 10→100 % was killed at 13 A with the current still rising; 15 A clears the peak.</p>
<figure><img src="{f_slam}" alt="Full slam responses with raised guards"><figcaption>10→90 % (sag 15 %, surge 12 A) and 10→100 % (sag 17.5 %, surge 15 A) land in 80–90 ms with peaks of 10.0 and 13.0 A. From 5 % (~140 eHz) AM32's staged limits hold duty at the startup cap (400) and then the low-RPM limit (800) while speed builds, so full duty arrives at +390 ms and the inrush peaks lower, at 10.9 A.</figcaption></figure>
{table(["slam", "eHz before", "eHz settled", "settle ms (±5 %)", "peak mA", "min bus mV", "desyncs", "sag / surge guard", "run"], slam_rows)}
<p class="note">Margins on this pack at this charge: about 1.2 points of sag, 2 A of surge and 0.7 V above the 9 V floor on the 10→100 slam. Measurement range is not the limit: the DRV8304H current amplifiers clip near 23 A per phase.</p>

<h2>5 · The low end</h2>
<p>3 → 4 → 5 → 4 → 3 % after the standard walk-in to 10 %, 4 s each, recorded at 50 ms; then 10 s single dwells at 5, 10 and 15 % to count lost control ticks.</p>
<figure><img src="{f_low}" alt="Low-end profile and lost control ticks"><figcaption>3–4 % from below runs the open-loop sine stepper (no BLDC duty, ~250 mA). Coming up to 5 % hands over to polling mode, which had not locked into interrupt mode within 4 s. The operating map's 5 % dwell, entered from 10 %, held 138 eHz in interrupt mode. Right: ticks lost per 10 s dwell.</figcaption></figure>
{table(["throttle %", "duty", "mode", "mA"], low_rows)}
{table(["throttle %", "duty", "ticks lost", "late (>1.5 periods)", "worst gap µs", "COMP entries / s", "camp re-entries / s"], lost_rows)}
<p class="note">At 5 % throttle (~140 eHz) the half-interval gate stays closed for most of a long step. A comparator already sitting at its post-crossing level makes the priority-0 COMP handler re-enter continuously until the gate opens, starving the 20 kHz control tick (priority 2). AM32's handler camps the same way. The cost is protection latency: up to 0.64 ms without a control tick at 5 %; none lost from 15 % up.</p>

<h2>6 · Starts</h2>
<p>Three cold starts with the standard sequence (sine ramp, walk-in 5–10 % at 1 s per step, hold 10 %), and three that go straight from the sine ramp to 20 %, all recorded at 10 ms from the start of the sine ramp.</p>
<figure><img src="{f_st}" alt="Start transients"><figcaption>Standard starts change over to BLDC at 4.1 s every time and lock into interrupt mode by 7.1 s at about 440 mA. Direct starts lock within 0.3 s of changeover, at up to 1.4 A.</figcaption></figure>
{table(["start", "BLDC from (s)", "interrupt-mode lock (s)", "peak mA", "desyncs", "run"], st_rows)}
<p class="note">A fourth direct start (recorder check run, same control code, <code>{esc(pathlib.Path(check[-1]).name) if check else '—'}</code>) did not lock within 2.7 s: polling mode at the startup duty cap with repeated BEMF timeouts. Direct starts: 3 of 4 locked. Intermittent; mechanism open.</p>

<h2>7 · Events and timing budget</h2>
<h3>Events at 100 % (image <code>765fcf9a</code>, stage 7)</h3>
{table(["event", "re-hold eHz", "mA", "desyncs in re-hold", "capture"], [[e[0], e[1], fmt(e[2]), e[3], esc(e[4])] for e in ev])}
<p>Coast-down speed: bridge cut at the end of a hold, phase A crossings timed while the rotor coasts. 100 %: rotor {c100:.0f} eHz, loop {l100:.0f} eHz ({100 * (l100 / c100 - 1):+.1f} %). 80 %: rotor {c80:.0f}, loop {l80:.0f} ({100 * (l80 / c80 - 1):+.1f} %). Hard down-step 100 → 25 % with damped drive on the battery: 0 desyncs, re-hold 1141 eHz.</p>
<h3>Interrupt budget (static longest path, report image)</h3>
{table(["root", "NVIC prio", "instructions", "AM32", "ratio", "µs at 2 wait states"], wcet_rows)}
<p>{'<br>'.join(esc(b) for b in budget)}</p>
<p>Measured control tick at 100 %: mean {h100.get('t6mean', '—')} cycles of 3200, 0 overruns, worst tick gap {h100.get('gapmax', '—')} cycles, 0 ticks lost (capture <code>{esc(pathlib.Path(h100.get('file', '')).name)}</code>).</p>

<h2>8 · Findings</h2>
<ul>
<li><strong>Speed is real rotor speed.</strong> Coast timing puts the loop within 1–2 % of the coasting rotor at 80 and 100 %.</li>
<li><strong>Full slams are guard-limited, not control-limited.</strong> The duty ramp matches AM32's G071 defaults; a 10→90 command lands in about 13 ms. At 10 % sag / 12 A surge the bench guard kills the slam with the loop locked; at 17.5 % / 15 A, 10→90, 10→100 and 5→100 % all hold with zero desyncs. Production AM32 has no such stops.</li>
<li><strong>Lost control ticks at 5 % throttle</strong> from comparator camping (section 5). Same handler shape as AM32; worth a look if protections ever need tighter latency at idle-like speeds.</li>
<li><strong>Direct start into 20 %: 1 of 4 failed to lock</strong> within 2.7 s. The standard walk-in start locked 3 of 3.</li>
<li><strong>5 % throttle has hysteresis:</strong> entered from above it runs in interrupt mode at ~140 eHz; entered from the sine band it sits in polling.</li>
<li><strong>Phase A falling is accepted early</strong>, about 23 µs, in 6–11 % of revolutions at 80–87.5 % duty before the commutation-path cut and under 1 % after it. The trigger is a board or motor property: firmware50 shows the same phase with the same sign.</li>
</ul>

<h2>9 · Caveats and provenance</h2>
<ul>
<li>Section 4b ran on images <code>1c7e7935</code> (sag 15 %) and <code>0cbc3373</code> (sag 17.5 %, surge 15 A): the report image with only the bench-guard constants changed.</li>
<li>Report runs (sections 1, 3–6) used image <code>{REPORT_IMAGE}</code>: the stage-7 control code (<code>765fcf9a</code>) plus the bench recorder; interrupt figures are identical (section 7). Section 2 holds and the section 7 events ran on the stage-7 images named in their rows.</li>
<li>Throttle is the bench command in percent; applied duty is what the firmware reports (0–2000). Speed is computed from the commutation interval and is unreliable in polling mode and meaningless in the sine stepper (shown as 0).</li>
<li>Current is the 16 ms average of the firmware's per-millisecond metered reading. It has not been compared to a meter for rm32 on this rig.</li>
<li>One bench, one motor, one battery, one session each. Single runs except the 100 % holds, starts and events (3 each).</li>
<li>Recorder time comes from the CPU's free-running SysTick, so it stays true when control ticks are lost; host segment times are aligned to it within the command latency (~0.1–0.2 s).</li>
</ul>
<h3>Captures</h3>
<ul class="prov">{sources}</ul>
</main>
"""
    pathlib.Path(a.out).write_text(page, encoding="utf-8")
    print(f"wrote {a.out} ({len(page) / 1e6:.2f} MB), {len(SOURCES)} sources")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
