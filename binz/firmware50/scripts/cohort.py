#!/usr/bin/env python3
"""Behavior reference for the refactor goal (notebook E109+): the E101-E108
25% cohort, and the per-run / per-rung gates the fixture enforces.

Cohort: every reason-2 25% capture from E101, E104, E107 and E108 (the images
of the frozen behavior). For each of the four quantities the goal names --
accepted count, rate identity, coast eHz and current proxy -- the spread is
the cohort's [min, max]; a new 25% run "holds behavior" when all four sit
inside it (``check``).

Gates (shared with ``bemf_run.py``):

* run gates 1-3: stop reason 2 (deadline), hold >= 30 s at target, forced 0,
  **one** rate identity -- the non-circular one (E172 replaced the pair of
  tests, of which the first measured only truncation) --
  accepted rate within 1% of the loop's expectation AND of 6 x coast eHz,
  `unstable` non-zero and the blanking gate witnessed (``too_early`` non-zero,
  or ``blank_arms`` non-zero once E134 enforces the gate with the line masked),
  coast eHz within 5% of the oracle,
  and **coast crossings > 0**. A coast's crossings are the comparator
  transitions the bridge-off witness counted (``COASTTIMING trans``);
  ``BEMFCOAST crossings`` is an ADC count on pins no longer scanned (E041)
  and is always 0, so it cannot be the witness.
* rung oracle comparison: the rung's mean coast eHz and mean current proxy
  within 20% of the oracle's at that duty.

Usage:
    python scripts/cohort.py                 # print the cohort spread
    python scripts/cohort.py --check FILE    # one 25% capture against it
"""

from __future__ import annotations

import argparse
import pathlib
import statistics
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
CAPS = REPO / "captures"

COHORT_GLOBS = ["e101-qual25_*.txt", "e104-qual25_*.txt", "e107-qual25_*.txt", "e108-qual25_*.txt"]

# The oracle's figures per duty (tenths): eHz and current proxy mA. Same table
# as bemf_run.py's ORACLE (DUTY_50_CAMPAIGN.md rung table).
# Campaign 6 (E137) adds the 27.5-37.5% rungs. 300/350 are measured rows of
# the same table; the half-rungs 275/325/375 are linear interpolations of its
# neighbours and are marked here so no entry claims to be measured.
ORACLE = {100: (401, 45), 150: (704, 58), 200: (941, 167), 250: (1186, 326),
          275: (1279, 423), 288: (1327, 473), 300: (1371, 519), 325: (1468, 630),
          338: (1519, 691), 350: (1564, 740), 375: (1650, 843), 400: (1736, 945),
          425: (1815, 1091), 450: (1893, 1237), 475: (1995, 1420), 500: (2096, 1603)}
ORACLE_INTERPOLATED = (275, 288, 325, 338, 375, 425, 475)


def fields(line: str) -> dict[str, str]:
    out = {}
    for tok in line.split()[1:]:
        if "=" in tok:
            k, v = tok.split("=", 1)
            out[k] = v
    return out


def coast_ehz(iv: list[int]) -> int:
    """The rotor's speed **at the bridge-off instant**, from the coast.

    The firmware reports eight half-periods after the bridge floats, and the
    rotor decelerates across them, so any average of them under-reads the
    speed the loop was actually holding. Campaign 6 (notebook E143) measured
    that bias over 360 captures: the median of the pair sums, which this
    function used through campaign 6, puts the rate-identity ratio's centre at
    **1006.7** with a standard deviation of 2.6 -- three thousandths from the
    gate's own edge, so **37 of 360 runs (10%) fell outside a 1% band that
    nothing was wrong with**. Two of this campaign's "failures" were that
    artefact (E140's 35% cohort, E142's 15% run 02).

    So: drop the first half-period, which can be the demagnetisation transient
    rather than the rotor (E113: 461 µs against 423-445 for the rest), fit a
    least-squares line through the remaining pair sums against pair index, and
    read it back at the instant before the first retained pair. The same 360
    captures then centre at **1002.7**, standard deviation 2.9, with **4**
    outside the band.

    The band is unchanged at +-1%; only the bias is removed. Every campaign-6
    result was judged on the biased figure, which reads high, so each one was
    held to a stricter test than this and none of them is weakened."""
    def pair_sums(h: list[int]) -> list[int]:
        return [h[i] + h[i + 1] for i in range(len(h) - 1) if h[i] + h[i + 1] > 0]

    ps = pair_sums(iv[1:]) or pair_sums(iv)
    n = len(ps)
    if n == 0:
        return 0
    if n < 3:
        # Too short to fit: the median is all this is good for.
        return round(1e6 / statistics.median(ps))
    mx = (n - 1) / 2
    my = sum(ps) / n
    sxx = sum((x - mx) ** 2 for x in range(n))
    sxy = sum((x - mx) * (y - my) for x, y in enumerate(ps))
    if sxx == 0:
        return round(1e6 / my)
    at_float = my + (sxy / sxx) * (-0.5 - mx)
    return round(1e6 / at_float) if at_float > 0 else 0


def parse(path: pathlib.Path) -> dict | None:
    """The quantities a run is judged on, or None if the capture is incomplete."""
    rec: dict[str, dict[str, str]] = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        for key in (
            "BEMFRUN",
            "BEMFDONE",
            "BEMFGATE",
            "BEMFRATE",
            "BEMFTAIL",
            "BEMFCURRENT",
            "COASTTIMING",
            "BEMFDRIVEN",
        ):
            if line.startswith(key + " "):
                rec[key] = fields(line)  # the last occurrence wins (restart: seg 2)
    if not all(k in rec for k in ("BEMFRUN", "BEMFDONE", "BEMFRATE", "BEMFCURRENT", "COASTTIMING")):
        return None
    iv = [int(x) for x in rec["COASTTIMING"].get("iv_us", "").split(",") if x]
    coast = coast_ehz(iv)
    zc = int(rec["BEMFRATE"].get("zc_per_s", 0))
    return {
        "file": path.name,
        "duty": int(rec["BEMFRUN"].get("target_duty_tenths", 0)),
        "reason": int(rec["BEMFDONE"].get("reason", 0)),
        "accepted": int(rec["BEMFDONE"].get("accepted", 0)),
        "forced": int(rec["BEMFDONE"].get("forced", 0)),
        "too_early": int(rec["BEMFDONE"].get("too_early", 0)),
        "blank_arms": int(rec.get("BEMFDRIVEN", {}).get("blank_arms", 0)),
        "unstable": int(rec["BEMFDONE"].get("unstable", 0)),
        "hold_ms": int(rec["BEMFRATE"].get("hold_ms", 0)),
        "rate_permille": int(rec["BEMFRATE"].get("zc_rate_permille_of_expected", 0)),
        "has_tail": bool(rec.get("BEMFTAIL")),
        **_rate_vs_coast(rec, zc, coast),
        "coast_ehz": coast,
        "coast_crossings": int(rec["COASTTIMING"].get("trans", 0)),
        "hold_ma": int(rec["BEMFCURRENT"].get("hold_ma", 0)),
        # E193: the duty the foldback governor ended at. Absent from captures
        # written before E187, in which case it is taken as the commanded duty
        # (those runs could not report it, and nothing else in them can say).
        "ceiling_tenths": int(rec["BEMFCURRENT"].get("ceiling_tenths", 0)),
        "worst_ma": int(rec["BEMFCURRENT"].get("worst_ma", 0)),
    }


def _rate_vs_coast(rec: dict, zc: int, coast: int) -> dict:
    """The non-circular rate identity, and which inputs produced it.

    Preferred: the matched powered window (`BEMFTAIL`) against the
    time-anchored coast fit (`speed.coast_fit`), both introduced in E155 --
    neither rounds to whole µs and both state their origin. Older captures
    carry neither, so they fall back to the whole-hold `zc_per_s` against six
    times the E143 index-fitted coast, which is what the gate used through
    campaigns 6-8; the source is recorded either way so no cohort mixes them
    silently.
    """
    import speed  # noqa: PLC0415  -- sibling script, imported lazily

    tail, ct = rec.get("BEMFTAIL", {}), rec.get("COASTTIMING", {})
    accepts, span = int(tail.get("accepts", 0)), int(tail.get("span_us", 0))
    iv = [int(x) for x in ct.get("iv_us", "0").split(",") if x]
    fit = speed.coast_fit(int(ct.get("offset_us", 0)), int(ct.get("first_us", 0)), iv) if iv else None
    if accepts and span and fit:
        powered = accepts * 1e6 / (6.0 * span)
        at_stop = fit[0]
        if at_stop > 0:
            return {
                "rate_vs_coast_permille": round(1000 * powered / at_stop),
                "rate_source": "matched window vs time-anchored coast",
            }
    if coast:
        return {
            "rate_vs_coast_permille": 1000 * zc // (6 * coast),
            "rate_source": "legacy: whole hold vs index-fitted coast",
        }
    return {"rate_vs_coast_permille": 0, "rate_source": "no coast"}


def run_gates(r: dict, min_hold_ms: int = 30_000) -> list[str]:
    """Gates 1-3 for one run; empty list = pass.

    `min_hold_ms` is the dwell a qualifying run must reach. An exploratory run
    (campaign 6, E137) is driven on a shorter window and is judged on every
    other gate with this lowered, never with a gate removed."""
    fails = []
    if r["reason"] != 2:
        # Campaign 8's two hard stops carry their own codes, so a run that hit
        # one says so instead of being a bare "reason != 2".
        named = {15: "exhausted commutation deadline (a late arm)",
                 16: "the blanking window latched a comparator edge"}
        why = named.get(r["reason"])
        fails.append(f"reason {r['reason']} != 2" + (f": {why}" if why else ""))
    if r["hold_ms"] < min_hold_ms:
        fails.append(f"hold {r['hold_ms']} ms < {min_hold_ms}")
    if r["forced"] != 0:
        fails.append(f"forced {r['forced']}")
    # **The run must have held the duty it asked for.** The foldback governor
    # ratchets the ceiling *down only* on an over-current block and never
    # reports it back to the caller, so before E187 a throttled run looked
    # identical to a clean one, and even after E187 the fixture did not read
    # the field: a run that touched the 4 A allowance, got cut to a lower duty
    # and then completed its window would have passed every gate here and been
    # recorded as a rung run (E193 SS1). `ceiling_tenths` of 0 means a capture
    # older than the field, which is not judged.
    if r["ceiling_tenths"] and r["ceiling_tenths"] != r["duty"]:
        fails.append(
            f"throttled: ceiling {r['ceiling_tenths']} != commanded {r['duty']} "
            "-- the governor cut the duty, so this is not a run at this rung"
        )
    # **`zc_rate_permille_of_expected` is NOT gated any more, because it
    # measures nothing.** With `hold_forced = 0` the firmware computes
    # `zc_per_s = accepts / hold_ms` and `zc_expected_per_s = 1e6 /
    # floor(hold_ms * 1000 / accepts)` -- both sides from the same two
    # numbers, so the ratio is identically `floor(S) / S` for the unrounded
    # mean sector S. At 47.5% S = 80.97 µs, and 80/80.97 = 988 permille: a
    # 1.2% "failure" that is pure truncation and gets worse as the sector
    # shrinks. Three runs were failed by it at 47.5% (E170) and two images
    # were compared through it. The field is still parsed and printed for
    # continuity; nothing decides on it (E172).
    #
    # The gate is the **non-circular** identity: accepted events over the
    # matched powered window against the rotor's own speed from the coast
    # that follows it. Origins: the window ends at the last accepted crossing
    # before the stop and is 1-2 `TAIL_WINDOW_US` long (`BEMFTAIL`, raw counts
    # and spans, no rounding); the coast is fitted over full electrical cycles
    # placed in time from the stop stamp via the firmware-measured
    # `offset_us + first_us` and evaluated at the stop. Coverage: the window
    # is the end of the hold, not its whole length. Uncertainty: the estimator
    # repeats to sd 2.8 permille over five runs of one image at one rung
    # within a session (E155), and shifts by ~5 permille between sessions
    # (E164) -- so the 1% band is about 3.5 sd of the within-session figure.
    # **The tolerance is unchanged at 1%.**
    if not 990 <= r["rate_vs_coast_permille"] <= 1010:
        fails.append(
            f"rate vs coast {r['rate_vs_coast_permille']} permille outside 1% "
            f"({r['rate_source']})"
        )
    # A silent fallback to the legacy inputs would compare unlike quantities,
    # so it is a stated failure rather than a note nobody sees on a passing
    # run (E174 found that `rate_source` only surfaced on failure).
    if r["rate_source"].startswith("legacy") and r.get("has_tail"):
        fails.append("rate fell back to the legacy inputs although BEMFTAIL was present")
    if r["rate_source"] == "no coast":
        fails.append("no coast: the rate identity could not be computed")
    # The detector must be seen refusing edges, not just accepting them. Two
    # refusal witnesses: the persistence filter (`unstable`) and the blanking
    # gate. From E134 the gate is enforced in hardware -- the line stays masked
    # to the floor -- so early edges no longer dispatch and `too_early` is
    # legitimately 0; `blank_arms` (one per sector) is then the gate's witness.
    if r["unstable"] == 0:
        fails.append("unstable is zero")
    if r["too_early"] == 0 and r["blank_arms"] == 0:
        fails.append("neither too_early nor blank_arms: the blanking gate is not witnessed")
    if r["coast_crossings"] == 0:
        fails.append("coast crossings = 0: the rotor was not witnessed turning")
    if r["worst_ma"] >= WORST_MA_CEILING:
        fails.append(
            f"worst block {r['worst_ma']} mA at or above {WORST_MA_CEILING}: "
            "within 5% of the firmware's own 4 A allowance, where AverageCurrent "
            "folds back and a foldback disqualifies the rung"
        )
    ref = ORACLE.get(r["duty"])
    if ref and abs(r["coast_ehz"] - ref[0]) * 100 > 5 * ref[0]:
        fails.append(f"coast {r['coast_ehz']} eHz outside 5% of {ref[0]}")
    elif r["duty"] in SELF_REF_RUNGS:
        # Above the oracle's last entry this check used to SILENTLY DISAPPEAR,
        # so a 525 run was judged on strictly fewer gates than a 475 one (both
        # E235 reviews, independently). The within-run identity replaces it.
        fails += self_ref_fails(r)
    elif not ref:
        fails.append(f"duty {r['duty']} has neither an oracle figure nor a "
                     "within-run reference: refusing to judge it on fewer "
                     "gates than a lower rung")
    return fails


def rung_current_note(runs: list[dict]) -> str:
    """The current-proxy comparison, REPORT-ONLY since E124 (operator
    decision): the signed-current proxy fails the reference image's own +-20%
    band (oracle 15%: 163 / -31 / 172 mA against its 58), so it cannot gate
    progression. Every electrical protection stays armed; only this
    uncalibrated comparison left the gate."""
    ref = ORACLE.get(runs[0]["duty"]) if runs else None
    if not ref:
        return ""
    ma = sum(r["hold_ma"] for r in runs) / len(runs)
    return f"current (report-only): mean {ma:.0f} mA vs oracle {ref[1]} ({100 * (ma - ref[1]) / ref[1]:+.0f}%)"


# Rungs above the historical oracle's last entry (500). The goal is explicit
# that "historical oracle comparisons end at 50%; higher-rung references must be
# independently established", so these are judged against the run's OWN rotor
# instead of a table: the E143 rate identity, `rate_vs_coast_permille`, which is
# the loop's switching rate as a per mille of 6x the time-anchored coast rate.
#
# Nothing about it is imported from a previous image or a previous campaign,
# which is what makes it admissible here where an extrapolated oracle would not
# be.
SELF_REF_RUNGS = (525, 550, 575, 600)

# The worst 10.1 ms current block a run may show and still be judged a pass.
#
# E239 predeclared "worst_ma above 3000 mA ends the batch", it was breached at
# 3174 mA, and the batch ran on to completion because the rule lived in a
# notebook entry and nothing enforced it. It lives here now.
#
# The value is the firmware's own allowance minus a margin, not a choice:
# `protection::RAW_LIMIT` is 4000 mA, where `AverageCurrent` folds back on the
# first over-block and stops on the second. A foldback sets
# `ceiling_tenths < duty`, which cannot count as qualification -- so the gate
# fails a run 5% BEFORE the firmware starts folding, which is the difference
# between measuring a limit and disqualifying a rung.
#
# It retroactively fails nothing: the corpus maximum at any rung is 3174.
WORST_MA_CEILING = 3800

# The band is the qualified 500 cohort's own spread, not a choice: 27 healthy
# runs give min 993, median 1001, max 1013. 980..1020 is generous against that,
# and the first three 525 runs measured 1002 / 996 / 999 (E239).
SELF_REF_LO = 980
SELF_REF_HI = 1020


def self_ref_fails(r: dict) -> list[str]:
    """The within-run rate identity, for a rung with no historical reference."""
    v = r.get("rate_vs_coast_permille")
    if not v:
        return [f"no within-run rate identity in {r['file']}: "
                "the coast or the hold window is missing, so this rung has no "
                "reference at all and cannot be judged"]
    if not SELF_REF_LO <= v <= SELF_REF_HI:
        return [f"rate/coast {v} per mille outside {SELF_REF_LO}..{SELF_REF_HI} "
                "(the qualified 500 cohort's own band)"]
    return []


def rung_oracle(runs: list[dict]) -> list[str]:
    """Oracle comparison on the rung's means (speed); empty list = pass.
    The current comparison is report-only (`rung_current_note`, E124)."""
    if not runs:
        return ["no runs"]
    duty = runs[0]["duty"]
    if duty in SELF_REF_RUNGS:
        # Judged on every run's own identity rather than a mean against a
        # table: a mean would let one bad run hide behind two good ones, and
        # there is no external figure to compare a mean against anyway.
        fails = []
        for r in runs:
            fails += self_ref_fails(r)
        return fails
    ref = ORACLE.get(duty)
    if not ref:
        return [f"no oracle figure at duty {duty}"]
    ehz = sum(r["coast_ehz"] for r in runs) / len(runs)
    ma = sum(r["hold_ma"] for r in runs) / len(runs)
    fails = []
    if abs(ehz - ref[0]) > 0.2 * ref[0]:
        fails.append(f"mean coast {ehz:.0f} eHz vs oracle {ref[0]} (>20%)")
    del ma  # report-only: see rung_current_note
    return fails


# `rate_permille` is deliberately absent: it is the circular metric (E172),
# and `check()` banded it here long after `run_gates` stopped gating on it --
# which made "nothing gates on it" false, as the step-1 review found (E174).
QUANTITIES = ("accepted", "rate_vs_coast_permille", "coast_ehz", "hold_ma")


def cohort() -> list[dict]:
    runs = []
    for g in COHORT_GLOBS:
        for p in sorted(CAPS.rglob(g)):
            r = parse(p)
            if r and r["reason"] == 2 and r["duty"] == 250:
                runs.append(r)
    return runs


def spread(runs: list[dict]) -> dict[str, tuple[int, int]]:
    return {q: (min(r[q] for r in runs), max(r[q] for r in runs)) for q in QUANTITIES}


def check(path: pathlib.Path) -> list[str]:
    r = parse(path)
    if r is None:
        return ["incomplete capture"]
    fails = run_gates(r)
    sp = spread(cohort())
    for q in QUANTITIES:
        lo, hi = sp[q]
        if not lo <= r[q] <= hi:
            fails.append(f"{q}={r[q]} outside cohort [{lo}, {hi}]")
    return fails


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", type=pathlib.Path)
    args = ap.parse_args()
    runs = cohort()
    sp = spread(runs)
    print(f"COHORT n={len(runs)} files={','.join(r['file'] for r in runs)}")
    for q in QUANTITIES:
        print(f"  {q}: [{sp[q][0]}, {sp[q][1]}]")
    if args.check:
        fails = check(args.check)
        print(f"CHECK {args.check.name}: {'PASS' if not fails else 'FAIL'}")
        for f in fails:
            print(f"  - {f}")
        return 0 if not fails else 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
