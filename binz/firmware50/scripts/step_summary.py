#!/usr/bin/env python3
"""ENV-93+: per-run wall indicators for an envelope-push step.

    python scripts/step_summary.py LABEL [LABEL...]

Per capture (captures/2026-*/LABEL-*_01.txt): duty, stop reason, ceiling, metered hold/slow/fast max, foldbacks,
coast and loop eHz, mean interval, and the timing indicators a wall would move: late_arms, late_max_us,
com_late_max_us, track_max_us (largest accepted-event gap the tracking watch saw), gap_max_us (guard tick gap),
and spent_max_us / ci_min_us when the image has isr_stats=1. Refuses a capture without BEMFDONE (no silent skip).
"""
import glob, re, sys

KEYS = ["duty_tenths", "ceiling_tenths", "hold_ma", "ews_hold_max_ma", "ewf_hold_max_ma", "fold_slow", "fold_fast",
        "coast_ehz", "ehz_from_sector", "mean_ci_us", "late_arms", "late_max_us", "com_late_max_us", "track_max_us",
        "gap_max_us", "isr_stats", "spent_max_us", "ci_min_us"]
SHORT = ["duty", "ceil", "hold", "slow", "fast", "fS", "fF", "coast", "loop", "ci", "late", "lmax", "clate", "trk",
         "gap", "st", "spent", "cimin"]
print("run".ljust(22), "rsn", " ".join(s.rjust(5) for s in SHORT))
for label in sys.argv[1:]:
    for f in sorted(glob.glob(f"captures/2026-*/{label}-*_01.txt")):
        t = open(f, errors="replace").read()
        name = f.replace("\\", "/").split("/")[-1][:-7]
        m = re.search(r"BEMFDONE reason=(\d+)", t)
        if not m:
            print(name.ljust(22), "INCOMPLETE (no BEMFDONE)")
            continue
        vals = []
        for k in KEYS:
            mm = re.search(r"\b" + k + r"=(-?\d+)", t)
            vals.append(mm.group(1) if mm else "-")
        print(name.ljust(22), m.group(1).rjust(3), " ".join(v.rjust(5) for v in vals))
