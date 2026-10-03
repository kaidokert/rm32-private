#!/usr/bin/env python3
"""Parser for `char-capture` captures (study 2026-10-01). One wire format, one parser.

`load(path)` returns a dict:

* `report`: every `KEY=VALUE` of the run's report lines, keyed by line tag
  (`BEMFDONE`, `BEMFCURRENT`, `BEMFSAG`, `COASTTIMING`, ...).
* `profile`: the `{...}` spec the runner sent (from the `# profile` footer).
* `snap`: the `CHARSNAP` fields (`startup_us`, `closed`, ...).
* `rec`: recorder arrays, one entry per sample, all physical units:
  `t_s` (since the loop closed; 0 before), `duty_pct`, `amps` (metered scale,
  ENV-92 line), `bus_v`, `ehz` (from the published average commutation
  interval, 1e6 / (6 * avg_us)), `accepts` (accepted crossings in the sample).
* `hist`: 6 lists of 64 counts, |interval - mean of the last six| in 1 µs bins,
  per sector, counted in the settled window only.
* `late`: 64 counts of COM-service lateness (TIM16 entry minus the instant it
  was scheduled for) in 1 µs bins, commutation purpose only, settled window.
* `count`: `accepts`, `excursions` (interval >= 1.5x the mean of the last six),
  `late_arms` (the wait was already spent when COMP armed), `services`.
* `acc`: tail ring rows `(at_us, wait_us, step, stage, late)`, oldest first.
* `svc`: tail ring rows `(sched_us, late_us, step, purpose)`, oldest first.

Conversions (constants named once, here):
* metered mA = (residual * 4e6 + 116 * 1000 * RAW_LIMIT) / (RAW_LIMIT * 1131), residual = zero_start
  - 100 * (per-scan shunt sum); `protection::metered_ma`.
* bus V = code * VDDA / 4095 * 11.94, VDDA = 3000 * VCAL / ref_vref (VCAL 1662, the factory word).
"""

from __future__ import annotations

import pathlib
import re

RAW_LIMIT = 31_857
GAIN_X1000 = 1131
OFFSET_MA = 116
VCAL = 1662
BUS_DIV = 11.94
LATE_US = 5  # a "late commutation": service entry >= this many µs after its scheduled instant

KV = re.compile(r"(\w+)=(-?\d+)")


def metered_amps(residual: float) -> float:
    return (residual * 4_000_000 + OFFSET_MA * 1000 * RAW_LIMIT) / (RAW_LIMIT * GAIN_X1000) / 1000.0


def load(path) -> dict:
    p = pathlib.Path(path)
    text = p.read_text(encoding="utf-8", errors="replace")
    d = {"name": p.name, "path": str(p), "report": {}, "rec": {}, "hist": [], "late": [], "acc": [], "svc": [],
         "snap": {}, "count": {}, "profile": ""}
    raw = []
    for line in text.splitlines():
        if line.startswith("# profile "):
            d["profile"] = line[len("# profile "):].strip()
            continue
        tag = line.split(" ", 1)[0]
        if tag == "CHARREC":
            raw.append([int(x) for x in line.split()[1:]])
        elif tag == "CHARHIST":
            f = [int(x) for x in line.split()[1:]]
            d["hist"].append(f[1:])
        elif tag == "CHARLATE":
            d["late"] = [int(x) for x in line.split()[1:]]
        elif tag == "CHARACC":
            at, wait, flag = (int(x) for x in line.split()[1:])
            d["acc"].append((at, wait, flag & 7, (flag >> 4) & 3, bool(flag & 0x80)))
        elif tag == "CHARSVC":
            sched, late, flag = (int(x) for x in line.split()[1:])
            d["svc"].append((sched, late, flag & 7, (flag >> 4) & 15))
        elif tag in ("CHARSNAP", "CHARCOUNT", "CHARSVCCOUNT"):
            kv = {k: int(v) for k, v in KV.findall(line)}
            (d["snap"] if tag == "CHARSNAP" else d["count"]).update(kv)
        elif tag.isupper() and "=" in line and not tag.startswith("CHAR"):
            d["report"].setdefault(tag, {}).update({k: int(v) for k, v in KV.findall(line)})
    rep = d["report"]
    zero = rep.get("BEMFCURRENT", {}).get("zero_start")
    vref = rep.get("BEMFSAG", {}).get("ref_vref")
    if raw and zero and vref:
        vdda = 3000.0 * VCAL / vref
        d["rec"] = {
            "t_s": [r[0] / 1e6 for r in raw],
            "duty_pct": [r[1] / 10.0 for r in raw],
            "amps": [metered_amps(zero - 100 * r[2]) for r in raw],
            "bus_v": [r[3] * vdda / 4095.0 * BUS_DIV / 1000.0 for r in raw],
            "ehz": [1e6 / (6 * r[4]) if r[4] else 0.0 for r in raw],
            "accepts": [r[5] for r in raw],
        }
    d["vdda"] = 3000.0 * VCAL / vref if vref else None
    return d


def bus_volts(d: dict, code: int) -> float:
    return code * d["vdda"] / 4095.0 * BUS_DIV / 1000.0


def reason(d: dict) -> int | None:
    return d["report"].get("BEMFDONE", {}).get("reason")


def pct(hist, q: float) -> float | None:
    """The q-quantile of a 1 µs-bin histogram (bin k covers [k, k+1)); None if empty."""
    n = sum(hist)
    if not n:
        return None
    target = q * n
    acc = 0
    for k, c in enumerate(hist):
        acc += c
        if acc >= target:
            return k + 0.5
    return len(hist) - 0.5


def late_per_1k(d: dict) -> float | None:
    n = sum(d["late"])
    return 1000.0 * sum(d["late"][LATE_US:]) / n if n else None


def per_1k(d: dict, key: str) -> float | None:
    a = d["count"].get("accepts", 0)
    return 1000.0 * d["count"].get(key, 0) / a if a else None
