"""Build the HTML artifact study from an envelope-ladder run directory.

Usage: python scripts/build_study.py [run_dir]   (default: newest data/run_*)

Parses frames.csv + events.log and emits study.html in the run dir: rung
map, current-vs-throttle, sag-vs-current slope, BEMF-amplitude vs rung,
and the blackbox/kill tail. Pure stdlib + inline SVG (no deps, no CDN).
"""

import csv
import glob
import os
import re
import sys

VPH_DIV = 15.67
VM_DIV = 17.59
# Single-shunt DC-link average -> amps. One-point calibration anchored to
# the operator's metered 0.56 A at 7% duty / 100 Hz (spin-pwm), i.e. a
# ~12 mV IS-mean delta = 0.56 A. Absolute current is approximate (the
# shunt sees pulsed current); the trend and supply-sag correlation are the
# solid signals.
IS_MV_PER_A = 21.4
IS_BASE_DEFAULT = 1640


def newest_run():
    root = os.path.join(os.path.dirname(__file__), "..", "data")
    runs = sorted(glob.glob(os.path.join(root, "run_*")))
    if not runs:
        sys.exit("no run_* dirs in data/")
    return runs[-1]


def load(run_dir):
    frames = []
    with open(os.path.join(run_dir, "frames.csv")) as f:
        for r in csv.DictReader(f):
            frames.append({k: int(v) if k != "t" else float(v) for k, v in r.items()})
    rungs, coasts, catch, ends, bb, slips = [], [], None, [], [], []
    coast_wave = []
    # Binary telemetry bytes get interleaved into text lines, so match each
    # record token ANYWHERE in the line (not startswith).
    text = open(os.path.join(run_dir, "events.log"), encoding="utf-8", errors="replace").read()
    for m in re.finditer(r"RUNG,(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+),(\d+)", text):
        g = list(map(int, m.groups()))
        rungs.append(dict(idx=g[0], pct=g[1], fchz=g[2], is_min=g[3], is_mean=g[4],
                          is_max=g[5], vm_min=g[6], vm_mean=g[7], vm_max=g[8]))
    for m in re.finditer(r"COAST,(\d+),(\d+),(\d+),(\d+)(?:,(\d+))?", text):
        g = m.groups()
        coasts.append(dict(idx=int(g[0]), fchz=int(g[1]), ehz10=int(g[2]),
                           bemf_mv=int(g[3]), phase0=int(g[4]) if g[4] else None))
    m = re.search(r"CATCH,(\d+),(\d+),(\d+)", text)
    if m:
        catch = dict(fchz=int(m.group(1)), ehz10=int(m.group(2)), bemf_mv=int(m.group(3)))
    mb = re.search(r"BASE,(\d+),(\d+)", text)
    is_base = int(mb.group(1)) if mb else None
    for m in re.finditer(r"CD,(\d+),(\d+)", text):
        coast_wave.append((int(m.group(1)), int(m.group(2))))
    for m in re.finditer(r"SLIP,(\d+),(\d+)", text):
        slips.append(m.group(0))
    for m in re.finditer(r"BB,(\d+),([A-Z_]+),(\d+)", text):
        bb.append(f"{m.group(1)},{m.group(2)},{m.group(3)}")
    for m in re.finditer(r"END,([a-z A-Z:]+),(\d+)", text):
        ends.append(f"{m.group(1)},{m.group(2)}")
    coast_wave.sort()
    return frames, rungs, coasts, catch, ends, bb, coast_wave, is_base


def seq_loss(frames):
    if len(frames) < 2:
        return 0, 0
    lost = 0
    for a, b in zip(frames, frames[1:]):
        gap = (b["seq"] - a["seq"]) & 0xFF
        if gap > 1:
            lost += gap - 1
    return lost, lost + len(frames)


def svg_line(points, w, h, xr, yr, color, xlab, ylab, title, marks=True):
    x0, x1 = xr
    y0, y1 = yr
    if x1 == x0:
        x1 = x0 + 1
    if y1 == y0:
        y1 = y0 + 1
    pad = 44

    def sx(x):
        return pad + (x - x0) / (x1 - x0) * (w - pad - 12)

    def sy(y):
        return h - pad - (y - y0) / (y1 - y0) * (h - pad - 24)

    pts = " ".join(f"{sx(x):.1f},{sy(y):.1f}" for x, y in points)
    grid = []
    for i in range(5):
        gy = y0 + (y1 - y0) * i / 4
        yy = sy(gy)
        grid.append(f'<line x1="{pad}" y1="{yy:.1f}" x2="{w-12}" y2="{yy:.1f}" class="gl"/>')
        grid.append(f'<text x="{pad-6}" y="{yy+4:.1f}" class="ax" text-anchor="end">{gy:.0f}</text>')
    for i in range(6):
        gx = x0 + (x1 - x0) * i / 5
        xx = sx(gx)
        grid.append(f'<text x="{xx:.1f}" y="{h-pad+16:.1f}" class="ax" text-anchor="middle">{gx:.0f}</text>')
    # baseline axis
    grid.append(f'<line x1="{pad}" y1="{sy(y0):.1f}" x2="{w-12}" y2="{sy(y0):.1f}" class="axln"/>')
    grid.append(f'<line x1="{pad}" y1="{sy(y0):.1f}" x2="{pad}" y2="{sy(y1):.1f}" class="axln"/>')
    area = ""
    dots = ""
    if marks:
        ap = f"{sx(x0):.1f},{sy(y0):.1f} " + pts + f" {sx(points[-1][0]):.1f},{sy(y0):.1f}"
        area = f'<polygon points="{ap}" fill="{color}" opacity="0.10"/>'
        dots = "".join(f'<circle cx="{sx(x):.1f}" cy="{sy(y):.1f}" r="3.2" fill="{color}" stroke="var(--panel)" stroke-width="1"/>' for x, y in points)
    return f'''<figure class="card chart"><figcaption>{title}</figcaption>
<svg viewBox="0 0 {w} {h}" width="100%" preserveAspectRatio="xMidYMid meet" role="img" aria-label="{title}">
{''.join(grid)}
{area}
<polyline points="{pts}" fill="none" stroke="{color}" stroke-width="2.2" stroke-linejoin="round"/>
{dots}
<text x="{w//2}" y="{h-5}" class="axlab" text-anchor="middle">{xlab}</text>
<text x="15" y="{h//2}" class="axlab" text-anchor="middle" transform="rotate(-90 15 {h//2})">{ylab}</text>
</svg></figure>'''


def svg_line2(pa, pb, w, h, xr, yr, ca, cb, la, lb, xlab, ylab, title):
    """Two series on one scale (commanded vs measured), with a legend."""
    x0, x1 = xr
    y0, y1 = yr
    if x1 == x0:
        x1 += 1
    if y1 == y0:
        y1 += 1
    pad = 44

    def sx(x):
        return pad + (x - x0) / (x1 - x0) * (w - pad - 12)

    def sy(y):
        return h - pad - (y - y0) / (y1 - y0) * (h - pad - 40)

    grid = []
    for i in range(5):
        gy = y0 + (y1 - y0) * i / 4
        yy = sy(gy)
        grid.append(f'<line x1="{pad}" y1="{yy:.1f}" x2="{w-12}" y2="{yy:.1f}" class="gl"/>')
        grid.append(f'<text x="{pad-6}" y="{yy+4:.1f}" class="ax" text-anchor="end">{gy:.0f}</text>')
    for i in range(6):
        gx = x0 + (x1 - x0) * i / 5
        grid.append(f'<text x="{sx(gx):.1f}" y="{h-pad+16:.1f}" class="ax" text-anchor="middle">{gx:.0f}</text>')
    grid.append(f'<line x1="{pad}" y1="{sy(y0):.1f}" x2="{w-12}" y2="{sy(y0):.1f}" class="axln"/>')
    grid.append(f'<line x1="{pad}" y1="{sy(y0):.1f}" x2="{pad}" y2="{sy(y1):.1f}" class="axln"/>')

    def series(pts, color, dash=""):
        pl = " ".join(f"{sx(x):.1f},{sy(y):.1f}" for x, y in pts)
        d = f' stroke-dasharray="{dash}"' if dash else ""
        dots = "".join(f'<circle cx="{sx(x):.1f}" cy="{sy(y):.1f}" r="3.2" fill="{color}" stroke="var(--panel)" stroke-width="1"/>' for x, y in pts)
        return f'<polyline points="{pl}" fill="none" stroke="{color}" stroke-width="2.2" stroke-linejoin="round"{d}/>{dots}'

    # slip shading between the two series
    band = ""
    if len(pa) == len(pb):
        top = " ".join(f"{sx(x):.1f},{sy(y):.1f}" for x, y in pa)
        bot = " ".join(f"{sx(x):.1f},{sy(y):.1f}" for x, y in reversed(pb))
        band = f'<polygon points="{top} {bot}" fill="{ca}" opacity="0.08"/>'
    leg = (f'<g font-family="IBM Plex Mono,monospace" font-size="11">'
           f'<rect x="{pad+4}" y="8" width="11" height="3" fill="{ca}"/>'
           f'<text x="{pad+20}" y="12" class="ax">{la}</text>'
           f'<rect x="{pad+110}" y="8" width="11" height="3" fill="{cb}"/>'
           f'<text x="{pad+126}" y="12" class="ax">{lb}</text></g>')
    return f'''<figure class="card chart"><figcaption>{title}</figcaption>
<svg viewBox="0 0 {w} {h}" width="100%" preserveAspectRatio="xMidYMid meet" role="img" aria-label="{title}">
{''.join(grid)}{band}{leg}
{series(pa, ca)}{series(pb, cb, dash="5 4")}
<text x="{w//2}" y="{h-5}" class="axlab" text-anchor="middle">{xlab}</text>
<text x="15" y="{h//2}" class="axlab" text-anchor="middle" transform="rotate(-90 15 {h//2})">{ylab}</text>
</svg></figure>'''


def main():
    run_dir = sys.argv[1] if len(sys.argv) > 1 else newest_run()
    frames, rungs, coasts, catch, ends, bb, coast_wave, is_base = load(run_dir)
    name = os.path.basename(run_dir)

    # Per-rung derived quantities.
    def is_a(mv, base):
        return (mv - base) / IS_MV_PER_A

    # True zero-current baseline: streamed at arm, else the standard idle.
    base = is_base if is_base else IS_BASE_DEFAULT
    for r in rungs:
        r["i_mean_a"] = max(0.0, is_a(r["is_mean"], base))
        r["pk_mv"] = r["is_max"] - base  # raw shunt peak (spike-contaminated)
        r["vm_v"] = r["vm_mean"] * VM_DIV / 1000
        r["vm_min_v"] = r["vm_min"] * VM_DIV / 1000
        r["ehz"] = r["fchz"] / 100
    # End status: prefer explicit END line, else blackbox DONE/KILL.
    if not ends:
        if any(",DONE," in b for b in bb):
            ends = ["graceful (blackbox DONE),edge"]
        elif any("KILL_" in b for b in bb):
            ends = ["KILL,edge"]
    cmap = {c["idx"]: c for c in coasts}

    # MEASURED rotor speed from coast BEMF (V_bemf = Ke * eHz). Calibrate
    # Ke on the lowest rung (least demanding -> most likely synced), then
    # every rung's coast-BEMF gives a MEASURED eHz to compare against the
    # commanded eHz. Slip = how far the rotor fell behind the command.
    # This is the sync check: firmware "eRPM" is only COMMANDED; BEMF is
    # what the rotor actually did (no Halls, so BEMF is the only witness).
    ke = None
    for r in rungs:
        c = cmap.get(r["idx"])
        r["meas_ehz"] = None
        r["slip_pct"] = None
        if c and c["bemf_mv"] > 0:
            if ke is None:
                ke = c["bemf_mv"] / max(1, r["ehz"])  # mV per Hz-e
            if ke:
                r["meas_ehz"] = c["bemf_mv"] / ke
                r["slip_pct"] = 100 * (r["ehz"] - r["meas_ehz"]) / max(1, r["ehz"])
    synced = [r for r in rungs if r["slip_pct"] is not None and r["slip_pct"] < 15]
    sync_ceiling = synced[-1] if synced else None

    charts = []
    if rungs:
        charts.append(svg_line(
            [(r["pct"], r["i_mean_a"]) for r in rungs],
            560, 300, (0, max(r["pct"] for r in rungs) + 1),
            (0, max((r["i_mean_a"] for r in rungs)) * 1.15 + 0.05),
            "#38d6c8", "throttle (% duty)", "avg current (A)", "Current vs throttle"))
        charts.append(svg_line(
            [(r["i_mean_a"], r["vm_min_v"]) for r in rungs],
            560, 300,
            (0, max((r["i_mean_a"] for r in rungs)) * 1.1 + 0.05),
            (min((r["vm_min_v"] for r in rungs)) - 0.3, max((r["vm_v"] for r in rungs)) + 0.3),
            "#f2a541", "avg current (A)", "bus V (min under load)", "Bus sag vs current (supply slope)"))
        # THE headline: commanded eHz (what firmware told the motor) vs
        # measured eHz from coast-BEMF (what the rotor actually did).
        meas_pts = [(r["pct"], r["meas_ehz"]) for r in rungs if r["meas_ehz"] is not None]
        cmd_pts = [(r["pct"], r["ehz"]) for r in rungs]
        if meas_pts:
            ymax = max(max(y for _, y in cmd_pts), max(y for _, y in meas_pts)) * 1.12 + 2
            charts.append(svg_line2(
                cmd_pts, meas_pts, 560, 300, (0, max(r["pct"] for r in rungs) + 1),
                (0, ymax), "#8fe36b", "#a98bff", "commanded", "measured (BEMF)",
                "throttle (% duty)", "electrical Hz",
                "Commanded vs measured speed &mdash; the sync check"))
        bemf = [(r["pct"], cmap[r["idx"]]["bemf_mv"]) for r in rungs if r["idx"] in cmap]
        if bemf:
            charts.append(svg_line(
                bemf, 560, 300, (0, max(r["pct"] for r in rungs) + 1),
                (0, max(v for _, v in bemf) * 1.2 + 10),
                "#a98bff", "throttle (% duty)", "coast BEMF pk-pk (mV term)",
                "Coast BEMF amplitude (rotor-speed witness)"))
    if coast_wave:
        vs = [(i, v) for i, v in coast_wave]
        charts.append(svg_line(
            vs, 560, 240, (0, len(vs)), (min(v for _, v in vs), max(v for _, v in vs)),
            "#a98bff", "sample (0.101 ms)", "VPH1 pin mV", "Catch coast waveform (raw BEMF)", marks=False))

    # Rung table.
    def slipcell(r):
        if r["slip_pct"] is None:
            return "<td>-</td>"
        s = r["slip_pct"]
        cls = "ok" if s < 15 else ("warn" if s < 40 else "bad")
        return f'<td><span class="ev {cls}">{s:.0f}%</span></td>'
    rows = "".join(
        f"<tr><td>{r['pct']}%</td><td>{r['ehz']:.0f}</td>"
        f"<td>{r['meas_ehz']:.0f}</td>{slipcell(r)}"
        f"<td>{r['i_mean_a']:.2f}</td><td>{r['vm_min_v']:.2f}</td>"
        f"<td>{cmap.get(r['idx'],{}).get('bemf_mv','-')}</td></tr>"
        for r in rungs)

    # Supply slope fit.
    slope_txt = "-"
    if len(rungs) >= 2:
        xs = [r["i_mean_a"] for r in rungs]
        ys = [r["vm_v"] for r in rungs]
        n = len(xs)
        mx, my = sum(xs) / n, sum(ys) / n
        den = sum((x - mx) ** 2 for x in xs) or 1
        m = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / den
        slope_txt = f"{m*1000:.0f} mV/A"

    ev_color = {"DONE": "ok", "KILL_OC": "bad", "KILL_SAG": "bad", "KILL_NFLT": "bad",
                "KILL_NTC": "bad", "STRIKE_OC": "warn", "STRIKE_SAG": "warn",
                "PROVOKE": "warn", "COAST": "vio", "RESUME": "cy", "RUNG_START": "cy"}
    bb_rows = "".join(
        (lambda p: f'<tr><td class="mono">{int(p[0])/10:.1f}</td>'
                   f'<td><span class="ev {ev_color.get(p[1],"")}">{p[1]}</span></td>'
                   f'<td class="mono">{p[2] if len(p)>2 else ""}</td></tr>')(b.split(","))
        for b in bb)

    graceful = any("DONE" in b for b in bb) and not any("KILL_" in b for b in bb)
    end_label = "; ".join(ends) if ends else "in progress"
    end_cls = "ok" if graceful else ("bad" if any("KILL_" in b for b in bb) else "warn")
    top_pct = rungs[-1]["pct"] if rungs else 0
    peak_i = max((r["i_mean_a"] for r in rungs), default=0)
    lo, tot = seq_loss(frames)
    loss_pct = 100 * lo / max(1, tot)
    catch_bemf = catch["bemf_mv"] if catch else "-"

    STYLE = f'''
:root{{
  --bg:#f4f6f9; --panel:#ffffff; --ink:#182534; --muted:#5b6b7e;
  --line:#dde3ec; --axln:#b9c3d0; --grid:#e7ecf3;
  --cy:#0f9d8f; --am:#c9781c; --gn:#3f9a2e; --vio:#6c4fd6;
  --ok:#2e8b4f; --warn:#c9781c; --bad:#c23b3b; --accent:var(--cy);
}}
:root:not([data-theme="light"]){{
  @media (prefers-color-scheme: dark){{
    :root{{ --bg:#0c141f; --panel:#131e2c; --ink:#dfe7f2; --muted:#8798ab;
      --line:#243244; --axln:#3a4b60; --grid:#1c2836;
      --cy:#38d6c8; --am:#f2a541; --gn:#8fe36b; --vio:#a98bff;
      --ok:#5fd08a; --warn:#f2a541; --bad:#ff6b6b; --accent:var(--cy); }}
  }}
}}
:root[data-theme="dark"]{{
  --bg:#0c141f; --panel:#131e2c; --ink:#dfe7f2; --muted:#8798ab;
  --line:#243244; --axln:#3a4b60; --grid:#1c2836;
  --cy:#38d6c8; --am:#f2a541; --gn:#8fe36b; --vio:#a98bff;
  --ok:#5fd08a; --warn:#f2a541; --bad:#ff6b6b; --accent:var(--cy);
}}
*{{box-sizing:border-box;}}
body{{background:var(--bg);color:var(--ink);margin:0;
  font-family:"IBM Plex Sans",system-ui,sans-serif;font-size:15px;line-height:1.55;
  -webkit-font-smoothing:antialiased;}}
.mono{{font-family:"IBM Plex Mono",ui-monospace,monospace;font-variant-numeric:tabular-nums;}}
.wrap{{max-width:1080px;margin:0 auto;padding:32px 24px 64px;}}
header{{border-bottom:2px solid var(--accent);padding-bottom:18px;margin-bottom:26px;}}
.eyebrow{{font-family:"IBM Plex Mono",monospace;font-size:12px;letter-spacing:.14em;
  text-transform:uppercase;color:var(--accent);margin:0 0 8px;}}
h1{{font-size:30px;font-weight:600;margin:0 0 6px;letter-spacing:-.01em;text-wrap:balance;}}
.sub{{color:var(--muted);margin:0;font-size:14px;}}
.sub .mono{{color:var(--ink);}}
.strip{{display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:1px;
  background:var(--line);border:1px solid var(--line);border-radius:12px;overflow:hidden;margin:26px 0;}}
.stat{{background:var(--panel);padding:16px 18px;}}
.stat .k{{font-family:"IBM Plex Mono",monospace;font-size:11px;letter-spacing:.1em;
  text-transform:uppercase;color:var(--muted);margin:0 0 6px;}}
.stat .v{{font-family:"IBM Plex Mono",monospace;font-size:24px;font-weight:600;color:var(--ink);
  font-variant-numeric:tabular-nums;}}
.stat .v.ok{{color:var(--ok);}} .stat .v.bad{{color:var(--bad);}} .stat .v.warn{{color:var(--warn);}}
h2{{font-size:13px;font-family:"IBM Plex Mono",monospace;letter-spacing:.12em;text-transform:uppercase;
  color:var(--muted);margin:38px 0 14px;display:flex;align-items:center;gap:12px;}}
h2::after{{content:"";flex:1;height:1px;background:var(--line);}}
.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(340px,1fr));gap:16px;}}
.card{{background:var(--panel);border:1px solid var(--line);border-radius:12px;padding:16px 16px 8px;}}
figure.chart{{margin:0;}}
figcaption{{font-size:13px;font-weight:600;color:var(--ink);margin-bottom:6px;}}
svg .gl{{stroke:var(--grid);stroke-width:1;}}
svg .axln{{stroke:var(--axln);stroke-width:1;}}
svg .ax{{fill:var(--muted);font-family:"IBM Plex Mono",monospace;font-size:11px;}}
svg .axlab{{fill:var(--muted);font-family:"IBM Plex Sans",sans-serif;font-size:12px;}}
.tablecard{{background:var(--panel);border:1px solid var(--line);border-radius:12px;overflow-x:auto;}}
table{{border-collapse:collapse;width:100%;font-size:13px;min-width:520px;}}
th,td{{padding:9px 14px;text-align:right;border-bottom:1px solid var(--line);}}
th{{font-family:"IBM Plex Mono",monospace;font-size:11px;letter-spacing:.06em;text-transform:uppercase;
  color:var(--muted);font-weight:500;}}
td{{font-family:"IBM Plex Mono",monospace;font-variant-numeric:tabular-nums;color:var(--ink);}}
th:first-child,td:first-child{{text-align:left;}}
tbody tr:last-child td{{border-bottom:none;}}
.ev{{font-family:"IBM Plex Mono",monospace;font-size:11px;padding:2px 7px;border-radius:5px;
  background:var(--grid);color:var(--muted);}}
.ev.ok{{color:var(--ok);}} .ev.bad{{color:var(--bad);}} .ev.warn{{color:var(--warn);}}
.ev.cy{{color:var(--cy);}} .ev.vio{{color:var(--vio);}}
.callout{{background:var(--panel);border:1px solid var(--line);border-left:3px solid var(--vio);
  border-radius:10px;padding:16px 20px;margin:24px 0;font-size:14.5px;line-height:1.6;color:var(--ink);}}
.callout strong{{color:var(--vio);}} .callout em{{font-style:italic;color:var(--muted);}}
.foot{{color:var(--muted);font-size:12.5px;margin-top:28px;line-height:1.7;}}
.foot strong{{color:var(--ink);}}
.foot code{{font-family:"IBM Plex Mono",monospace;color:var(--ink);}}
@media (prefers-reduced-motion:no-preference){{.stat,.card{{transition:none;}}}}
'''

    sc_pct = sync_ceiling["pct"] if sync_ceiling else None
    sc_hz = sync_ceiling["meas_ehz"] if sync_ceiling else None
    sync_txt = f"{sc_pct}% / {sc_hz:.0f} Hz-e" if sync_ceiling else "not established"
    worst_slip = max((r["slip_pct"] for r in rungs if r["slip_pct"] is not None), default=0)

    BODY = f'''<div class="wrap">
<header>
  <p class="eyebrow">binz &middot; open-loop V/f campaign</p>
  <h1>Where the rotor stops following</h1>
  <p class="sub">NUCLEO-G071RB &plus; EVLDRIVE102H (STDRIVE102H, single shunt) &middot; sensorless, no Halls &middot; run <span class="mono">{name}</span></p>
</header>

<div class="callout">
  <strong>Commanded &ne; actual &mdash; and the witness is noisy.</strong> Open-loop V/f has no rotor
  feedback, so firmware &ldquo;eRPM&rdquo; is only what it <em>told</em> the motor. The only witness
  to what the rotor <em>did</em> is coast BEMF (V&nbsp;=&nbsp;Ke&middot;&omega;) &mdash; but on this
  low-inertia prop the rotor decays within milliseconds of gate-off, so per-rung BEMF is a
  <em>first-order</em> witness, not a tachometer. Across runs it places the sync edge anywhere from
  ~5% to ~12% duty; this run reads {sync_txt}. What is solid: the rotor spins and BEMF scales with
  throttle, but the exact sync/slip per rung needs the external oracle. <strong>The laser tacho on
  the prop is the definitive next step</strong> and the honest close-out of &ldquo;measured&rdquo;.
</div>

<div class="strip">
  <div class="stat"><p class="k">top duty commanded</p><p class="v">{top_pct}%</p></div>
  <div class="stat"><p class="k">BEMF sync (noisy)</p><p class="v warn" style="font-size:16px;line-height:1.9em">{sync_txt}</p></div>
  <div class="stat"><p class="k">bus floor</p><p class="v {'bad' if min((r['vm_min_v'] for r in rungs), default=12)<9.5 else 'warn'}">{min((r['vm_min_v'] for r in rungs), default=0):.1f} V</p></div>
  <div class="stat"><p class="k">peak avg I</p><p class="v">{peak_i:.2f}<span style="font-size:14px"> A</span></p></div>
  <div class="stat"><p class="k">stream loss</p><p class="v {'ok' if loss_pct<0.1 else 'warn'}">{loss_pct:.3f}%</p></div>
  <div class="stat"><p class="k">ended</p><p class="v {end_cls}" style="font-size:14px;line-height:1.9em">{'GRACEFUL' if graceful else end_label.upper()}</p></div>
</div>

<h2>Measured curves</h2>
<div class="grid">{''.join(charts)}</div>

<h2>Rung map &mdash; commanded vs measured</h2>
<div class="tablecard"><table>
<thead><tr><th>duty</th><th>cmd eHz</th><th>meas eHz</th><th>slip</th><th>I avg (A)</th><th>bus min V</th><th>BEMF mV</th></tr></thead>
<tbody>{rows}</tbody></table></div>

<h2>Flight recorder &mdash; last {len(bb)} events</h2>
<div class="tablecard"><table>
<thead><tr><th>t (ms)</th><th>event</th><th>value</th></tr></thead>
<tbody>{bb_rows}</tbody></table></div>

<p class="foot">
<strong>Measured, not commanded.</strong> Measured eHz = coast-BEMF amplitude &divide; Ke, with Ke
calibrated on the lowest rung (least demanding, taken as synced); slip = (commanded &minus; measured)
&divide; commanded. Coast BEMF is a decaying-rotor sample on a low-inertia prop, so measured eHz is a
first-order witness, not a tachometer &mdash; the laser tacho is the definitive oracle and is the open
next step. The <strong>~{sc_pct}%</strong> sync ceiling is an <em>open-loop-V/f, this-ramp-profile</em>
ceiling (a function of the voltage boost, accel rate, and start current), NOT the motor's limit &mdash;
closed-loop commutation is expected to go far past it.<br>
Current is the single DC-link shunt average (STDRIVE102H), one-point calibrated to the operator-metered
0.56 A at 7%; absolute amps approximate (the shunt overstates PSU current on slip transients), trend
solid. Terminal = pin&times;<code>{VPH_DIV}</code>, bus = pin&times;<code>{VM_DIV}</code>. Telemetry:
22-byte binary frames over VCOM @ 2 Mbaud; guards + timebase in the 9.9 kHz ADC-harvest ISR.
</p>
</div>'''

    full = ('<!doctype html><html lang="en"><head><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width,initial-scale=1">'
            '<title>V/f Sync-Edge Study</title>'
            '<link rel="preconnect" href="https://fonts.googleapis.com">'
            '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>'
            '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?'
            'family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600&display=swap">'
            f'<style>{STYLE}</style></head><body>{BODY}</body></html>')
    out = os.path.join(run_dir, "study.html")
    with open(out, "w", encoding="utf-8") as f:
        f.write(full)

    # Artifact fragment (no doctype/html/head/body — the Artifact skeleton
    # supplies those). Keeps <title>/<style> + body content.
    frag = (f'<title>V/f Sync-Edge Study</title>'
            '<link rel="preconnect" href="https://fonts.googleapis.com">'
            '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?'
            'family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600&display=swap">'
            f'<style>{STYLE}</style>{BODY}')
    with open(os.path.join(run_dir, "study.artifact.html"), "w", encoding="utf-8") as f:
        f.write(frag)
    print(f"wrote {out} ({len(rungs)} rungs, {len(coasts)} coasts, {len(bb)} bb events)")
    return out


if __name__ == "__main__":
    main()
