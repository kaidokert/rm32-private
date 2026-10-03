#!/usr/bin/env python3
"""Assemble the battery characterization study (one HTML page, figures embedded).

    python scripts/char_study.py

Runs the five section scripts (char_map, char_steps, char_ramps, char_lowend,
char_stops), each of which writes a PNG and a JSON table under
captures/char/fig/, then writes captures/char/study/fw50-battery-study.html.
The interpretive text (verdict, per-section notes, the AM32 comparison) lives in
captures/char/fig/prose.json, written after reading the data; every number in
the tables comes from the JSON the section scripts produced.
"""

from __future__ import annotations

import base64
import html
import json
import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
CAP = REPO / "captures" / "char"
FIG = CAP / "fig"
OUT = CAP / "study"


def run(script):
    r = subprocess.run([sys.executable, script], cwd=REPO / "scripts", capture_output=True, text=True)
    print(f"{script}: {r.stdout.strip()} {r.stderr.strip()[-400:]}")


def img(name):
    p = FIG / f"{name}.png"
    if not p.exists():
        return "<p class=note>figure not produced</p>"
    b = base64.b64encode(p.read_bytes()).decode()
    return f'<img alt="{name}" src="data:image/png;base64,{b}">'


def f(v, nd=0, unit=""):
    if v is None:
        return "–"
    if isinstance(v, bool):
        return "yes" if v else "no"
    if isinstance(v, (int, float)):
        return f"{v:.{nd}f}{unit}"
    return html.escape(str(v))


def table(head, rows, cls=None):
    th = "".join(f"<th>{html.escape(h)}</th>" for h in head)
    body = []
    for i, r in enumerate(rows):
        c = f' class="{cls[i]}"' if cls and cls[i] else ""
        body.append(f"<tr{c}>" + "".join(f"<td>{x}</td>" for x in r) + "</tr>")
    return f'<div class="tablewrap"><table><thead><tr>{th}</tr></thead><tbody>{"".join(body)}</tbody></table></div>'


def caps(names):
    return '<p class="caps">Captures: ' + ", ".join(f"<code>captures/char/{html.escape(n)}</code>" for n in names) + "</p>"


CSS = """
:root{--bg:#fbfbfa;--fg:#1d232b;--muted:#5d6672;--rule:#dde2e7;--card:#f1f4f6;--accent:#2a78d6;
--good:#1a7f4b;--warn:#a86a12;--bad:#b83227;--code:#eceff2;--codefg:#2b3442;}
@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){color-scheme:dark;--bg:#14181e;--fg:#dde3ea;--muted:#8b96a3;
--rule:#2a323c;--card:#1b222b;--accent:#5b9ce6;--good:#3dbb7e;--warn:#d9a441;--bad:#ef6b66;--code:#212831;--codefg:#c9d2dc;}}
:root[data-theme="dark"]{color-scheme:dark;--bg:#14181e;--fg:#dde3ea;--muted:#8b96a3;--rule:#2a323c;--card:#1b222b;--accent:#5b9ce6;
--good:#3dbb7e;--warn:#d9a441;--bad:#ef6b66;--code:#212831;--codefg:#c9d2dc;}
html{background:var(--bg);color:var(--fg)}
body{background:var(--bg);color:var(--fg);font:15px/1.55 "IBM Plex Sans","Segoe UI",system-ui,sans-serif;margin:0;padding:0 16px 80px}
main{max-width:900px;margin:0 auto}
h1{font:600 27px/1.2 "IBM Plex Sans",system-ui,sans-serif;margin:34px 0 4px;text-wrap:balance}
h2{font-size:19px;margin:46px 0 10px;padding-top:14px;border-top:1px solid var(--rule);text-wrap:balance}
h3{font-size:15px;margin:20px 0 6px}
p{margin:9px 0;max-width:72ch}
.sub{color:var(--muted);font-size:13.5px}
.verdict{background:var(--card);border:1px solid var(--rule);border-radius:8px;padding:14px 18px;margin:20px 0}
.verdict ol{margin:6px 0 2px;padding-left:22px}.verdict li{margin:6px 0}
figure{margin:16px 0}figure img{max-width:100%;height:auto;border:1px solid var(--rule);border-radius:6px;background:#fff}
figcaption,.caps{font-size:12.5px;color:var(--muted);margin-top:6px}
.tablewrap{overflow-x:auto}
table{border-collapse:collapse;font-size:13px;margin:12px 0;font-variant-numeric:tabular-nums}
th,td{padding:5px 10px;text-align:right;border-bottom:1px solid var(--rule);white-space:nowrap}
th{font-weight:600;color:var(--muted);font-size:12px}th:first-child,td:first-child{text-align:left}
tr.stop td{color:var(--bad)}tr.fold td{color:var(--warn)}
code{font-family:"IBM Plex Mono",Consolas,monospace;background:var(--code);color:var(--codefg);padding:1px 5px;border-radius:3px;font-size:12px}
dl.defs{display:grid;grid-template-columns:max-content 1fr;gap:6px 16px;font-size:13.5px}
dl.defs dt{font-weight:600}dl.defs dd{margin:0;color:var(--fg)}
.note{font-size:13px;color:var(--muted);border-left:3px solid var(--rule);padding:2px 0 2px 12px;margin:12px 0}
"""


def main():
    for s in ("char_map.py", "char_steps.py", "char_ramps.py", "char_lowend.py", "char_stops.py"):
        run(s)
    J = {k: json.load(open(FIG / f"{k}.json")) for k in ("map", "steps", "ramps", "lowend", "stops") if (FIG / f"{k}.json").exists()}
    P = json.load(open(FIG / "prose.json", encoding="utf-8")) if (FIG / "prose.json").exists() else {}
    h = []
    # The artifact publisher supplies the document skeleton: title and style go first.
    h.append(f"<title>firmware50 on battery</title><link rel=stylesheet href='https://fonts.googleapis.com/css2?family=IBM+Plex+Mono&family=IBM+Plex+Sans:wght@400;600&display=swap'>"
             f"<style>{CSS}</style><main>")
    h.append("<h1>firmware50 on battery — characterization study</h1>")
    h.append(f"<p class=sub>{P.get('subtitle', '')}</p>")
    h.append("<div class=verdict><strong>Verdict in five lines</strong><ol>" + "".join(f"<li>{x}</li>" for x in P.get("verdict", [])) + "</ol></div>")
    h.append("<h2>Definitions</h2><dl class=defs>" + "".join(f"<dt>{html.escape(k)}</dt><dd>{v}</dd>" for k, v in P.get("defs", [])) + "</dl>")
    # 1 map
    m = J.get("map", {"rows": [], "captures": []})
    h.append("<h2>1 · Operating map, 5–100 %</h2>" + P.get("map", ""))
    h.append(f"<figure>{img('map')}<figcaption>Produced by <code>scripts/char_map.py</code>. Red bands: rungs that stopped or folded.</figcaption></figure>")
    rows, cls = [], []
    for r in m["rows"]:
        stop = r["reason"] != 2
        fold = r["ceiling"] < r["duty"]
        cls.append("stop" if stop else ("fold" if fold else ""))
        p99 = [x for x in r["p99"] if x is not None]
        p50 = [x for x in r["p50"] if x is not None]
        rows.append([f(r["duty"], 0, " %"), f(r["loop_ehz"]), f(r["coast_ehz"]), f(r["hold_a"], 2), f(r["slow_a"], 2), f(r["bus_v"], 2),
                     f(max(p50) if p50 else None, 1), f(max(p99) if p99 else None, 1), f(r["late_1k"], 1), f(r["exc_1k"], 2), f(r["arm_1k"], 2),
                     "ok" if not stop else f"stop {r['reason']}"])
    h.append(table(["duty", "loop eHz", "coast eHz", "hold A", "slow peak A", "bus V", "worst-sector p50 µs", "worst-sector p99 µs",
                    "late /1k", "excursions /1k", "late arms /1k", "end"], rows, cls))
    h.append(caps(m["captures"]))
    # 2 steps
    s = J.get("steps", {"rows": [], "captures": []})
    h.append("<h2>2 · Step response</h2>" + P.get("steps", ""))
    h.append(f"<figure>{img('steps')}<figcaption>Produced by <code>scripts/char_steps.py</code>; three runs per step, time 0 = the commanded step.</figcaption></figure>")
    rows, cls = [], []
    for r in s["rows"]:
        cls.append("" if r.get("held") else "stop")
        rows.append([f"{r['a'] / 10:g}→{r['b'] / 10:g} %", r["k"], f(r.get("before")), f(r.get("after")), f(r.get("t90_ms")),
                     f(r.get("over_pct"), 1), f(r.get("peak_a"), 2), f(r.get("dip_v"), 2), f(r.get("held")),
                     "" if r["reason"] == 2 else f"stop {r['reason']}"])
    h.append(table(["step", "run", "eHz before", "eHz after", "t90 ms", "overshoot %", "peak A", "bus dip V", "lock held", "end"], rows, cls))
    h.append(caps(s["captures"]))
    # 3 ramps
    rp = J.get("ramps", {"rows": [], "captures": []})
    h.append("<h2>3 · Ramps 10 → 100 → 10 %</h2>" + P.get("ramps", ""))
    h.append(f"<figure>{img('ramps')}<figcaption>Produced by <code>scripts/char_ramps.py</code>. Solid: up leg; dotted: down leg.</figcaption></figure>")
    rows = [[r["slew"], f(r["reached"], 1, " %"), f(r["peak_a"], 2), f(r["hyst_50"]), f(r["hyst_max"]), r["dropouts"], r["folds"],
             r["reason_name"]] for r in rp["rows"]]
    h.append(table(["slew", "max duty", "peak A", "hysteresis @50 % eHz", "max |hysteresis| eHz", "dropout samples", "folds", "end"], rows))
    h.append(caps(rp["captures"]))
    # 4 low end
    lo = J.get("lowend", {"low": [], "restart": [], "captures": []})
    h.append("<h2>4 · Low end and restart</h2>" + P.get("lowend", ""))
    h.append(f"<p><strong>Lowest duty holding lock for 30 s: {f(lo.get('lowest_held'), 1, ' %')}</strong></p>")
    rows = [[f(r["duty"], 1, " %"), f(r["ran_s"], 1), f(r["ehz"]), f(r["hold_a"], 3), "held 30 s" if r["held"] else f"stop {r['reason']}"]
            for r in sorted(lo["low"], key=lambda r: -r["duty"])]
    h.append(table(["duty", "closed loop s", "eHz (hold)", "A (hold)", "result"], rows))
    h.append(f"<figure>{img('lowend')}<figcaption>Produced by <code>scripts/char_lowend.py</code>: restart from stop, traces placed after each run's measured startup.</figcaption></figure>")
    rows = [[f(r["duty"], 0, " %"), r["k"], f(r["startup_s"], 2), f(r.get("t90_close_s"), 2), f(r.get("restart_s"), 2), f(r.get("final_ehz")),
             f(r.get("stop_after_close_s", None) and r["stop_after_close_s"] * 1000, 1), "ok" if r["reason"] == 2 else f"stop {r['reason']}"]
            for r in sorted(lo["restart"], key=lambda r: (r["duty"], r["k"]))]
    h.append(table(["duty", "run", "startup s", "close → 90 % s", "restart total s", "final eHz", "stop after close ms", "end"], rows))
    h.append(caps(lo["captures"]))
    # 5 stops
    sp = J.get("stops", {"events": [], "captures": []})
    h.append("<h2>5 · Protections under real events</h2>" + P.get("stops", ""))
    if sp["events"]:
        h.append(f"<figure>{img('stops')}<figcaption>Produced by <code>scripts/char_stops.py</code> from each run's frozen accept ring.</figcaption></figure>")
    rows = [[html.escape(e["file"]), e["reason"], html.escape(e["cls"]), e["folds"], f(e["median_iv"]), f(e["max_iv"]), e["late"],
             f(e["steady"])] for e in sp["events"]]
    h.append(table(["capture", "reason", "class", "folds", "median interval µs", "max interval µs", "late arms", "steady before"], rows))
    h.append(caps(sp["captures"]))
    h.append("<h2>What the map says</h2>" + P.get("conclusion", ""))
    h.append("<h2>Against AM32's documented behaviour</h2>" + P.get("am32", ""))
    h.append(f"<p class=sub>{P.get('footer', '')}</p></main>")
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "fw50-battery-study.html").write_text("".join(h), encoding="utf-8")
    print(f"wrote {OUT / 'fw50-battery-study.html'}")


if __name__ == "__main__":
    main()
