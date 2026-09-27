"""Unloaded capture checks; reuse safeguards without the prop-loaded oracle.

No firmware thresholds are changed. Missing evidence fails closed. The existing
±1% matched-window identity is retained, including the historical no-prop50
failure. This module cannot write to the loaded ladder.
"""
from pathlib import Path

import cohort

# New captures must not inherit cohort.py's permissive legacy defaults.
REQUIRED = {
    "BEMFRUN": ("target_duty_tenths",),
    "BEMFDONE": ("reason", "forced", "accepted", "unstable", "too_early"),
    "BEMFGATE": ("target_tenths",),
    "BEMFRATE": ("hold_ms", "hold_accepted", "hold_forced", "zc_per_s"),
    "BEMFTAIL": ("accepts", "span_us", "start_before_stop_us", "end_before_stop_us"),
    "BEMFCURRENT": ("hold_ma", "worst_ma", "ceiling_tenths", "duty_tenths",
                    "applied_cap", "applied_ccr"),
    "BEMFSAG": ("ref_bus", "ref_vref", "filt_bus", "filt_vref", "tripped"),
    "BEMFGUARD": ("track_max_us", "gap_max_us", "loop_gap_max_us"),
    "BEMFRCOMP": ("late_arms", "thin_count"),
    "COASTTIMING": ("trans", "offset_us", "first_us", "iv_us"),
}


def request_errors(command: str, duty: int, pre: str, override: bool) -> list[str]:
    """Only the reviewed no-injection commands; validate before any flash/write."""
    fails = []
    if override:
        fails.append("no step-check or loaded-ladder anchor in unloaded mode")
    if command not in ("9", "8", "l", "L"):
        fails.append("unloaded mode supports only gentle9/8 or climb l/L")
    if not 0 < duty <= 800:
        fails.append("unloaded request outside campaign ceiling")
    gentle = {"9": 150, "8": 250}
    if command in gentle and (duty != gentle[command] or pre):
        fails.append(f"gentle{command} must request{gentle[command]} without pre-keys")
    if command in ("l", "L") and (not pre or set(pre) - set("+-")):
        fails.append("climb requires non-driving +/- pre-keys and verified CLIMBAT")
    return fails


def absolute_climb_pre(duty: int) -> str:
    """Reset from any authorized state, then select an exact 2.5% rung."""
    if not 375 <= duty <= 800 or (duty - 375) % 25:
        raise ValueError("unsupported absolute climb rung")
    return "-" * 19 + "+" * ((duty - 375) // 25)


def verdict(path: Path, duty: int, min_hold_ms: int, *, period: int = 1333) -> list[str]:
    text = path.read_text(encoding="utf-8")
    records = {}
    preflights = []
    positions = {}
    off_positions = []
    for pos, line in enumerate(text.splitlines()):
        key = line.split(" ", 1)[0]
        if key.startswith("BEMF") or key in ("PREFLIGHT", "COASTTIMING"):
            if key not in ("PREFLIGHT", "BEMFDRVROWS") and key in records:
                return [f"duplicate record {key}; ambiguous run association"]
            records[key] = cohort.fields(line)
            positions[key] = pos
        if key == "PREFLIGHT":
            preflights.append(cohort.fields(line))
            off_positions.append(pos)
    missing = [f"missing {key}.{field}" for key, fields in REQUIRED.items()
               for field in fields if field not in records.get(key, {})]
    if missing:
        return missing
    if (len(off_positions) != 2 or len([s for s in text.splitlines() if s == "POSTSTOP"]) != 1 or
            not positions["BEMFRUN"] < off_positions[0] < text.splitlines().index("POSTSTOP") <
            off_positions[1] < positions["BEMFDONE"] < positions["COASTTIMING"]):
        return ["pre/post safe-off records do not bracket one run"]
    for key, fields in REQUIRED.items():
        for field in fields:
            if field != "iv_us":
                try:
                    int(records[key][field])
                except ValueError:
                    return [f"invalid integer {key}.{field}"]
    for key, fields in {
        "BEMFTAIL": ("accepts", "span_us"),
        "BEMFSAG": ("ref_bus", "ref_vref", "filt_bus", "filt_vref"),
    }.items():
        if any(int(records[key][field]) <= 0 for field in fields):
            reason = int(records["BEMFDONE"]["reason"])
            stop = [f"firmware stop reason {reason} != deadline2"] if reason != 2 else []
            return stop + [f"nonpositive {key} measurement; identity unavailable"]
    r = cohort.parse(path)
    if r is None:
        return ["incomplete capture"]
    tail = records["BEMFTAIL"]
    start, end, span = (int(tail[k]) for k in
                        ("start_before_stop_us", "end_before_stop_us", "span_us"))
    # hold_ms is truncated to milliseconds; permit only that documented <1ms
    # quantisation, not a tail extending outside the actual-target window.
    if not (0 <= end < start <= r["hold_ms"] * 1000 + 999 and span == start - end):
        return ["incoherent tail span/origins or tail outside hold"]
    guard = records["BEMFGUARD"]
    tail_age_budget = sum(int(guard[k]) for k in
                         ("track_max_us", "gap_max_us", "loop_gap_max_us"))
    if end > tail_age_budget:
        return ["powered tail is stale beyond tracking and observed service gaps"]
    fails = cohort.run_gates(r, min_hold_ms, propless=True)
    if r["rate_source"] != "matched window vs time-anchored coast":
        fails.append("matched-window identity required; legacy fallback forbidden")
    if duty <= 0 or r["duty"] != duty:
        fails.append(f"requested duty {duty} != captured {r['duty']}")
    for key in ("BEMFGATE", "BEMFCURRENT"):
        field = "target_tenths" if key == "BEMFGATE" else "duty_tenths"
        if int(records.get(key, {}).get(field, -1)) != duty:
            fails.append(f"{key} {field} does not match request")
    cur = records.get("BEMFCURRENT", {})
    if int(cur.get("ceiling_tenths", -1)) != duty:
        fails.append("target ceiling missing or folded back")
    if int(cur.get("applied_cap", 0)) < duty or int(cur.get("applied_ccr", 0)) <= 0:
        fails.append("applied duty evidence missing or below request cap")
    # Old 1333 archives predate carrier metadata. Every new capture declares
    # its expected period in the host header and independently in BEMFRUN.
    declared = records["BEMFRUN"].get("run_period_ticks")
    expected = f"# expected_run_period_ticks {period}"
    if period != 1333 or declared is not None or "# expected_run_period_ticks" in text:
        if (expected not in text.splitlines() or declared != str(period) or
                records["BEMFRUN"].get("startup_ticks") != "6400"):
            fails.append("carrier declaration missing or differs from expected build")
    if period not in (1000, 1333):
        fails.append("unreviewed running carrier period")
    # Never derive period from CCR: that could hide actual duty foldback.
    if int(cur["applied_ccr"]) != period * duty // 1000:
        fails.append(f"CCR does not equal requested duty on period {period}")
    if "applied_period" in cur and int(cur["applied_period"]) != period:
        fails.append("applied carrier does not equal target carrier")
    if "entry_period_ticks" in records["BEMFRUN"] and "applied_period" not in cur:
        fails.append("new carrier image omitted actual period metadata")
    if int(records["BEMFRATE"]["hold_forced"]) != 0:
        fails.append("forced commutation during hold")
    zero_fields = {
        "BEMFRCOMP": ("late_arms",),
        "BEMFDRIVEN": ("storm", "overrun", "blank_latched"),
        "BEMFGUARD": ("reason", "track_fault"),
        "BEMFSAG": ("tripped",),
    }
    for key, names in zero_fields.items():
        for name in names:
            if records.get(key, {}).get(name) != "0":
                fails.append(f"{key}.{name} nonzero or missing")
    off = dict(moe="0", ccr1="0", ccr2="0", ccr3="0", gates_low="1",
               en="0", nfault="1", verdict="PASS")
    if len(preflights) < 2 or any(any(p.get(k) != v for k, v in off.items())
                                 for p in (preflights[:1] + preflights[-1:])):
        fails.append("pre/post bridge-off verification missing or failed")
    return fails


def sag_dump_errors(path: Path) -> list[str]:
    """Complete versioned post-stop evidence, not a silent zero-row recorder."""
    import sag
    lines = path.read_text(encoding="utf-8").splitlines()
    if sum(line.strip() == "SAGEND" for line in lines) != 1:
        return ["missing or multiple SAGEND"]
    try:
        snap, rows, slow = sag.parse(path)
        sag.check_units(snap, rows)
    except (ValueError, KeyError, SystemExit) as exc:
        return [f"invalid sag dump: {exc}"]
    fails = []
    if not rows or snap.get("fast_len") != len(rows) or snap.get("slow_len") != len(slow):
        fails.append("empty or incomplete sag rings")
    if snap.get("frozen") != 1 or snap.get("judged", 0) < len(rows):
        fails.append("sag ring not frozen or invalid judgement count")
    if (snap.get("row_v"), snap.get("fine_hz"), snap.get("span16_us")) != (3, 8_000_000, 8192):
        fails.append("unexpected sag record units")
    if (snap.get("num"), snap.get("den"), snap.get("streak_to_latch")) != (95, 100, 3):
        fails.append("sag guard configuration differs from retained protection")
    return fails


def summary(path: Path) -> str:
    r = cohort.parse(path)
    if r is None:
        return "PROPLESS incomplete=1"
    return (f"PROPLESS load=none duty={r['duty']} hold_ms={r['hold_ms']} "
            f"reason={r['reason']} coast_ehz={r['coast_ehz']} "
            f"matched_rate_permille={r['rate_vs_coast_permille']} "
            f"current_proxy_ma={r['hold_ma']} ceiling={r['ceiling_tenths']} "
            f"bus_rest_permille={r['droop_permille']} "
            "loaded_oracle=not_applicable")
